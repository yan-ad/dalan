//! Native direct Redis RESP2 reader. Commands are JSON arrays of strings, never
//! SQL, shell text or URLs containing credentials. Use a read-only Redis ACL;
//! the allowlist is defense in depth, not server-side authorization.
//!
//! A small bounded RESP2 codec is intentional: ordinary Redis clients allocate
//! bulk/array replies before a preview limit can be applied. This codec rejects
//! replies over 8 MiB, 100000 elements or 32 nesting levels before allocating
//! their declared sizes. Every operation owns its socket (no pool or background
//! reader); cancellation closes it. TLS uses the original host and native-tls.
//! SCAN is not a snapshot: duplicates/order changes are possible. Binary or
//! oversized key names are omitted from the catalog. Hash/set/sorted-set browse
//! offsets are opaque SCAN cursors, not row numbers; an oversized advisory SCAN
//! batch is rejected rather than silently skipping entries.
use crate::{
    mysql::{
        BrowseRequest, CatalogSnapshot, CellValue, ColumnInfo, ConnectionReport, DatabaseCatalog,
        TableInfo, TablePage, bounded_with,
    },
    query::{QueryRequest, QueryResult},
    sources::{Authentication, ConnectionMode, DbEngine, SourceProfile, TlsMode, Transport},
};
use anyhow::{Result, anyhow, ensure};
use std::{collections::BTreeMap, future::Future, pin::Pin, time::Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

const WIRE_CAP: usize = 8 * 1024 * 1024;
const NODE_CAP: usize = 100_000;
const KEY_CAP: usize = 4096;
const CELL_CAP: usize = 4096;
const PAGE_CAP: usize = 2 * 1024 * 1024;
const OBJECT_CAP: usize = 50_000;
const COMMAND_CAP: usize = 64 * 1024;

fn database(value: &str) -> Result<u16> {
    ensure!(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        "Redis database must be an integer from 0 through 65535"
    );
    value
        .parse::<u16>()
        .map_err(|_| anyhow!("Redis database must be an integer from 0 through 65535"))
}
fn key(value: &str) -> Result<()> {
    ensure!(
        value.len() <= KEY_CAP,
        "Redis key or argument exceeds 4096 bytes"
    );
    Ok(())
}
fn resolved(profile: &SourceProfile) -> Result<SourceProfile> {
    ensure!(
        profile.engine == DbEngine::Redis,
        "This executor supports Redis only"
    );
    ensure!(
        matches!(profile.transport, Transport::Direct),
        "Redis supports direct transport only"
    );
    ensure!(
        !matches!(profile.endpoint, ConnectionMode::UnixSocket { .. }),
        "Redis Unix sockets are unsupported"
    );
    let p = profile.resolved()?;
    database(p.database.as_deref().unwrap_or("0"))?;
    Ok(p)
}

#[derive(Debug)]
enum Resp {
    Null,
    Bytes(Vec<u8>),
    Integer(i64),
    Array(Vec<Resp>),
}
struct Budget {
    bytes: usize,
    nodes: usize,
}
impl Budget {
    fn charge(&mut self, size: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(size)
            .ok_or_else(|| anyhow!("Redis reply exceeds size limit"))?;
        ensure!(self.bytes <= WIRE_CAP, "Redis reply exceeds 8 MiB limit");
        Ok(())
    }
}
fn io_error(_: std::io::Error) -> anyhow::Error {
    anyhow!("Redis transport I/O failed")
}
async fn line<R: AsyncRead + Unpin>(reader: &mut R, budget: &mut Budget) -> Result<Vec<u8>> {
    // Do not use read_until: malicious servers can send an unlimited header.
    let mut out = Vec::new();
    loop {
        budget.charge(1)?;
        let b = reader.read_u8().await.map_err(io_error)?;
        if b == b'\r' {
            budget.charge(1)?;
            ensure!(
                reader.read_u8().await.map_err(io_error)? == b'\n',
                "Invalid Redis protocol framing"
            );
            return Ok(out);
        }
        ensure!(
            b != b'\n' && out.len() < KEY_CAP,
            "Invalid or oversized Redis protocol header"
        );
        out.push(b);
    }
}
fn number(bytes: &[u8]) -> Result<i64> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("Invalid Redis protocol number"))
}
fn read_resp<'a, R: AsyncRead + Unpin + Send + 'a>(
    reader: &'a mut R,
    budget: &'a mut Budget,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Result<Resp>> + Send + 'a>> {
    Box::pin(async move {
        ensure!(
            depth <= 32 && budget.nodes < NODE_CAP,
            "Redis reply exceeds nesting or element limit"
        );
        budget.nodes += 1;
        budget.charge(1)?;
        let tag = reader.read_u8().await.map_err(io_error)?;
        let header = line(reader, budget).await?;
        match tag {
            b'+' => Ok(Resp::Bytes(header)),
            // Never surface server text: it can echo credentials or user data.
            b'-' => Err(anyhow!(
                "Redis rejected the operation; check ACL permissions, authentication and key type"
            )),
            b':' => Ok(Resp::Integer(number(&header)?)),
            b'$' => {
                let size = number(&header)?;
                if size == -1 {
                    return Ok(Resp::Null);
                }
                ensure!(
                    size >= 0 && size as u64 <= WIRE_CAP as u64,
                    "Invalid or oversized Redis bulk reply"
                );
                budget.charge(size as usize + 2)?;
                let mut bytes = vec![0; size as usize];
                reader.read_exact(&mut bytes).await.map_err(io_error)?;
                let mut end = [0; 2];
                reader.read_exact(&mut end).await.map_err(io_error)?;
                ensure!(end == *b"\r\n", "Invalid Redis bulk framing");
                Ok(Resp::Bytes(bytes))
            }
            b'*' => {
                let size = number(&header)?;
                if size == -1 {
                    return Ok(Resp::Null);
                }
                ensure!(
                    size >= 0 && size as u64 <= (NODE_CAP - budget.nodes) as u64,
                    "Redis reply exceeds element limit"
                );
                let mut values = Vec::with_capacity((size as usize).min(1024));
                for _ in 0..size {
                    values.push(read_resp(reader, budget, depth + 1).await?);
                }
                Ok(Resp::Array(values))
            }
            _ => Err(anyhow!("Unsupported Redis protocol reply (RESP2 required)")),
        }
    })
}
trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}
struct Session {
    stream: BufReader<Box<dyn Stream>>,
}
async fn pem(path: &str) -> Result<Vec<u8>> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|_| anyhow!("Cannot read Redis TLS file"))?;
        let mut bytes = Vec::new();
        file.take(WIRE_CAP as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow!("Cannot read Redis TLS file"))?;
        ensure!(bytes.len() <= WIRE_CAP, "Redis TLS file exceeds 8 MiB");
        Ok(bytes)
    })
    .await
    .map_err(|_| anyhow!("Redis TLS file loading failed"))?
}
async fn tls(profile: &SourceProfile) -> Result<tokio_native_tls::TlsConnector> {
    let mut builder = native_tls::TlsConnector::builder();
    builder.danger_accept_invalid_certs(profile.tls == TlsMode::Required);
    builder.danger_accept_invalid_hostnames(matches!(
        profile.tls,
        TlsMode::Required | TlsMode::VerifyCa
    ));
    if let Some(path) = &profile.ca_path {
        let bytes = pem(path).await?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| anyhow!("Invalid Redis CA PEM encoding"))?;
        let mut count = 0;
        for segment in text.split_inclusive("-----END CERTIFICATE-----") {
            if let Some(start) = segment.find("-----BEGIN CERTIFICATE-----") {
                builder.add_root_certificate(
                    native_tls::Certificate::from_pem(&segment.as_bytes()[start..])
                        .map_err(|_| anyhow!("Invalid Redis CA certificate"))?,
                );
                count += 1;
            }
        }
        ensure!(count > 0, "Redis CA file contains no PEM certificates");
    }
    if let (Some(cert), Some(key)) = (&profile.ssl_client_cert, &profile.ssl_client_key) {
        builder.identity(native_tls::Identity::from_pkcs8(&pem(cert).await?, &pem(key).await?)
            .map_err(|_| anyhow!("Invalid Redis TLS identity; use a certificate chain and unencrypted PKCS#8 PEM key"))?);
    }
    Ok(tokio_native_tls::TlsConnector::from(
        builder
            .build()
            .map_err(|_| anyhow!("Redis TLS configuration failed"))?,
    ))
}
impl Session {
    async fn connect(profile: &SourceProfile, password: &str) -> Result<Self> {
        bounded_with(profile.options.connect_timeout_seconds, async {
            ensure!(
                password.len() <= COMMAND_CAP,
                "Redis password exceeds size limit"
            );
            let socket = tokio::net::TcpStream::connect((profile.host.as_str(), profile.port))
                .await
                .map_err(io_error)?;
            let stream: Box<dyn Stream> = if profile.tls == TlsMode::Disabled {
                Box::new(socket)
            } else {
                Box::new(
                    tls(profile)
                        .await?
                        .connect(&profile.host, socket)
                        .await
                        .map_err(|_| {
                            anyhow!("Redis TLS handshake or identity verification failed")
                        })?,
                )
            };
            let mut session = Self {
                stream: BufReader::new(stream),
            };
            if profile.authentication == Authentication::UserPassword {
                let reply = session
                    .command(&["AUTH", &profile.username, password])
                    .await?;
                expect_ok(reply)?;
            }
            session
                .select(profile.database.as_deref().unwrap_or("0"))
                .await?;
            ensure!(
                bytes(session.command(&["PING"]).await?)? == b"PONG",
                "Unexpected Redis PING reply"
            );
            Ok(session)
        })
        .await
    }
    async fn select(&mut self, db: &str) -> Result<()> {
        let canonical = database(db)?.to_string();
        expect_ok(self.command(&["SELECT", &canonical]).await?)
    }
    async fn command(&mut self, args: &[&str]) -> Result<Resp> {
        let mut wire = format!("*{}\r\n", args.len()).into_bytes();
        for arg in args {
            wire.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
            wire.extend_from_slice(arg.as_bytes());
            wire.extend_from_slice(b"\r\n");
        }
        ensure!(
            wire.len() <= 2 * COMMAND_CAP,
            "Redis request exceeds size limit"
        );
        self.stream.write_all(&wire).await.map_err(io_error)?;
        self.stream.flush().await.map_err(io_error)?;
        read_resp(&mut self.stream, &mut Budget { bytes: 0, nodes: 0 }, 0).await
    }
}
fn bytes(resp: Resp) -> Result<Vec<u8>> {
    match resp {
        Resp::Bytes(v) => Ok(v),
        _ => Err(anyhow!("Unexpected Redis reply type")),
    }
}
fn array(resp: Resp) -> Result<Vec<Resp>> {
    match resp {
        Resp::Array(v) => Ok(v),
        _ => Err(anyhow!("Unexpected Redis reply type")),
    }
}
fn expect_ok(resp: Resp) -> Result<()> {
    ensure!(bytes(resp)? == b"OK", "Unexpected Redis acknowledgement");
    Ok(())
}
fn scan_reply(resp: Resp) -> Result<(u64, Vec<Resp>)> {
    let mut parts = array(resp)?;
    ensure!(parts.len() == 2, "Invalid Redis SCAN reply");
    let values = array(parts.pop().unwrap())?;
    let cursor = bytes(parts.pop().unwrap())?;
    let cursor = std::str::from_utf8(&cursor)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| anyhow!("Invalid Redis SCAN cursor"))?;
    Ok((cursor, values))
}

fn parse_command(text: &str) -> Result<Vec<String>> {
    ensure!(
        !text.is_empty() && text.len() <= COMMAND_CAP,
        "Redis command must contain 1 through 65536 bytes"
    );
    let mut args: Vec<String> = serde_json::from_str(text).map_err(|_| {
        anyhow!(
            "Expected one Redis command as a JSON array of strings, for example [\"GET\",\"key\"]"
        )
    })?;
    ensure!(
        !args.is_empty() && args.len() <= 128,
        "Redis command must contain 1 through 128 arguments"
    );
    for arg in &args {
        key(arg)?;
    }
    args[0] = args[0].to_ascii_uppercase();
    let n = args.len();
    let valid = match args[0].as_str() {
        "PING" => n == 1 || n == 2,
        "DBSIZE" => n == 1,
        "GET" | "TYPE" | "TTL" | "PTTL" | "STRLEN" | "HLEN" | "HGETALL" | "SCARD" | "SMEMBERS"
        | "LLEN" | "ZCARD" => n == 2,
        "MGET" | "EXISTS" => n >= 2,
        "HGET" => n == 3,
        "HMGET" => n >= 3,
        "LRANGE" => n == 4 && signed(&args[2]) && signed(&args[3]),
        "ZRANGE" => {
            (n == 4 || (n == 5 && args[4].eq_ignore_ascii_case("WITHSCORES")))
                && signed(&args[2])
                && signed(&args[3])
        }
        "SCAN" => scan_args(&args, 1),
        "HSCAN" | "SSCAN" | "ZSCAN" => scan_args(&args, 2),
        _ => false,
    };
    ensure!(
        valid,
        "Redis command or arguments are not in the read-only allowlist"
    );
    Ok(args)
}
fn signed(s: &str) -> bool {
    s.parse::<i64>().is_ok()
}
fn scan_args(args: &[String], index: usize) -> bool {
    if args.len() <= index || args[index].parse::<u64>().is_err() {
        return false;
    }
    let mut count = false;
    let mut pattern = false;
    let mut kind = false;
    let mut i = index + 1;
    while i < args.len() {
        if i + 1 >= args.len() {
            return false;
        }
        match args[i].to_ascii_uppercase().as_str() {
            "COUNT" if !count => {
                if !args[i + 1]
                    .parse::<u32>()
                    .is_ok_and(|n| (1..=200).contains(&n))
                {
                    return false;
                }
                count = true;
            }
            "MATCH" if !pattern => pattern = true,
            "TYPE" if !kind && index == 1 => {
                if !matches!(
                    args[i + 1].as_str(),
                    "string" | "hash" | "set" | "list" | "zset" | "stream"
                ) {
                    return false;
                }
                kind = true;
            }
            _ => return false,
        }
        i += 2;
    }
    true
}
/// Validate a single JSON string-array command. AUTH/SELECT and administrative,
/// scripting, blocking and mutating commands can never be supplied by the user.
pub fn validate_read_only(text: &str) -> Result<()> {
    parse_command(text).map(|_| ())
}

async fn database_names(session: &mut Session, profile: &SourceProfile) -> Result<Vec<String>> {
    if let Some(db) = &profile.database {
        return Ok(vec![database(db)?.to_string()]);
    }
    let info = bytes(session.command(&["INFO", "keyspace"]).await?)?;
    let text =
        std::str::from_utf8(&info).map_err(|_| anyhow!("Invalid Redis keyspace metadata"))?;
    let mut names = vec!["0".to_owned()];
    for line in text.lines() {
        if let Some((prefix, _)) = line.split_once(':')
            && let Some(db) = prefix.strip_prefix("db")
        {
            names.push(database(db)?.to_string());
        }
        ensure!(names.len() <= 1000, "Too many Redis databases");
    }
    names.sort_by_key(|n| n.parse::<u16>().unwrap_or(0));
    names.dedup();
    Ok(names)
}
async fn key_type(session: &mut Session, key: &str) -> Result<String> {
    let value = bytes(session.command(&["TYPE", key]).await?)?;
    let kind = std::str::from_utf8(&value).map_err(|_| anyhow!("Invalid Redis key type"))?;
    ensure!(
        matches!(
            kind,
            "none" | "string" | "hash" | "set" | "list" | "zset" | "stream"
        ),
        "Unsupported Redis key type"
    );
    Ok(kind.to_owned())
}
async fn key_tables(session: &mut Session, db: &str, cap: usize) -> Result<Vec<TableInfo>> {
    session.select(db).await?;
    let mut cursor = 0;
    let mut seen = 0;
    let mut iterations = 0;
    let mut tables = BTreeMap::new();
    loop {
        iterations += 1;
        ensure!(
            iterations <= OBJECT_CAP,
            "Redis catalog scan exceeds iteration limit"
        );
        let (next, values) = scan_reply(
            session
                .command(&["SCAN", &cursor.to_string(), "COUNT", "200"])
                .await?,
        )?;
        for value in values {
            seen += 1;
            ensure!(seen <= NODE_CAP, "Redis catalog scan exceeds work limit");
            let raw = bytes(value)?;
            if raw.len() > KEY_CAP {
                continue;
            }
            let Ok(name) = String::from_utf8(raw) else {
                continue;
            };
            if tables.contains_key(&name) {
                continue;
            }
            ensure!(
                tables.len() < cap,
                "Too many Redis keys (maximum 50000 total)"
            );
            let kind = key_type(session, &name).await?;
            if kind != "none" {
                tables.insert(
                    name.clone(),
                    TableInfo {
                        name,
                        kind: kind.to_ascii_uppercase(),
                    },
                );
            }
        }
        cursor = next;
        if cursor == 0 {
            break;
        }
    }
    Ok(tables.into_values().collect())
}
pub async fn test_connection(profile: &SourceProfile, password: &str) -> Result<ConnectionReport> {
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let mut session = Session::connect(&profile, password).await?;
        Ok(ConnectionReport {
            server_version: "Redis (native RESP2; version not queried)".into(),
            databases: database_names(&mut session, &profile).await?,
        })
    })
    .await
}
pub async fn tables(profile: &SourceProfile, password: &str, db: &str) -> Result<Vec<TableInfo>> {
    database(db)?;
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let mut session = Session::connect(&profile, password).await?;
        key_tables(&mut session, db, OBJECT_CAP).await
    })
    .await
}
pub async fn discover_catalog(profile: &SourceProfile, password: &str) -> Result<CatalogSnapshot> {
    let profile = resolved(profile)?;
    bounded_with(120, async {
        let mut session = Session::connect(&profile, password).await?;
        let names = bounded_with(
            profile.options.query_timeout_seconds,
            database_names(&mut session, &profile),
        )
        .await?;
        let mut remaining = OBJECT_CAP;
        let mut databases = Vec::new();
        for db in names {
            if !profile.visible_schema(&db) {
                continue;
            }
            let tables = bounded_with(
                profile.options.query_timeout_seconds,
                key_tables(&mut session, &db, remaining),
            )
            .await?;
            remaining -= tables.len();
            databases.push(DatabaseCatalog { name: db, tables });
        }
        Ok(CatalogSnapshot { databases })
    })
    .await
}
fn value_columns(kind: &str) -> Vec<ColumnInfo> {
    let names: &[&str] = match kind {
        "hash" => &["field", "value"],
        "zset" => &["member", "score"],
        "list" => &["index", "value"],
        _ => &["value"],
    };
    names
        .iter()
        .map(|n| ColumnInfo {
            name: (*n).into(),
            data_type: if *n == "index" || *n == "score" {
                "number"
            } else {
                "Redis bytes"
            }
            .into(),
            nullable: true,
            is_primary_key: false,
        })
        .collect()
}
pub async fn columns(
    profile: &SourceProfile,
    password: &str,
    db: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>> {
    database(db)?;
    key(table)?;
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let mut session = Session::connect(&profile, password).await?;
        session.select(db).await?;
        let kind = key_type(&mut session, table).await?;
        ensure!(
            kind != "none" && kind != "stream",
            "Redis key is missing or has an unsupported browse type"
        );
        Ok(value_columns(&kind))
    })
    .await
}

fn cell(value: Resp, truncated: &mut bool) -> CellValue {
    match value {
        Resp::Null => CellValue::Null,
        Resp::Integer(n) => CellValue::Number(n.to_string()),
        Resp::Bytes(v) => {
            if let Ok(text) = std::str::from_utf8(&v) {
                CellValue::Text(clip(text.to_owned(), truncated))
            } else {
                let cut = v.len() > (CELL_CAP - 2) / 2;
                *truncated |= cut;
                let mut text = String::from("0x");
                for b in v.iter().take((CELL_CAP - 2) / 2) {
                    use std::fmt::Write;
                    let _ = write!(text, "{b:02x}");
                }
                CellValue::Binary(text)
            }
        }
        Resp::Array(values) => {
            // Nested arrays retain native structure in a JSON preview. Binary
            // values use an explicit hex object, rather than lossy UTF-8.
            fn json(v: Resp) -> serde_json::Value {
                match v {
                    Resp::Null => serde_json::Value::Null,
                    Resp::Integer(n) => n.into(),
                    Resp::Array(a) => a.into_iter().map(json).collect(),
                    Resp::Bytes(b) => match String::from_utf8(b) {
                        Ok(s) => s.into(),
                        Err(e) => {
                            let text: String =
                                e.as_bytes().iter().map(|v| format!("{v:02x}")).collect();
                            serde_json::json!({"hex": text})
                        }
                    },
                }
            }
            CellValue::Text(clip(json(Resp::Array(values)).to_string(), truncated))
        }
    }
}
fn clip(mut text: String, truncated: &mut bool) -> String {
    if text.len() > CELL_CAP {
        *truncated = true;
        let mut end = CELL_CAP - 3;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push('…');
    }
    text
}
fn page(
    columns: Vec<ColumnInfo>,
    rows: Vec<Vec<Resp>>,
    offset: u64,
    limit: u32,
    next: Option<u64>,
) -> TablePage {
    let mut output = Vec::new();
    let mut truncated = false;
    let mut size = 0;
    let mut more = false;
    for row in rows {
        if output.len() >= limit as usize {
            more = true;
            break;
        }
        let cells: Vec<_> = row.into_iter().map(|v| cell(v, &mut truncated)).collect();
        let bytes: usize = cells.iter().map(|c| c.display().len()).sum();
        if size + bytes > PAGE_CAP {
            truncated = true;
            more = true;
            break;
        }
        size += bytes;
        output.push(cells);
    }
    let next_offset = next.or_else(|| more.then_some(offset + output.len() as u64));
    TablePage {
        columns,
        rows: output,
        offset,
        has_more: next_offset.is_some(),
        next_offset,
        truncated,
    }
}
pub async fn browse(
    profile: &SourceProfile,
    password: &str,
    request: &BrowseRequest,
) -> Result<TablePage> {
    ensure!(
        request.filter.is_none()
            && request.sort.is_none()
            && request.where_clause.trim().is_empty()
            && request.order_by.trim().is_empty(),
        "Redis browse does not support SQL filters or sorting; use a JSON command array in QueryConsole"
    );
    database(&request.database)?;
    key(&request.table)?;
    ensure!(
        (1..=200).contains(&request.limit),
        "Redis page size must be 1 through 200"
    );
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let mut session = Session::connect(&profile, password).await?;
        session.select(&request.database).await?;
        let kind = key_type(&mut session, &request.table).await?;
        let mut next = None;
        let rows = match kind.as_str() {
            "string" => {
                ensure!(request.offset == 0, "Redis string values have no paging offset");
                vec![vec![session.command(&["GET", &request.table]).await?]]
            }
            "list" => {
                ensure!(request.offset <= i64::MAX as u64 - 201, "Redis list offset exceeds range");
                let values = array(session.command(&["LRANGE", &request.table, &request.offset.to_string(), &(request.offset + u64::from(request.limit)).to_string()]).await?)?;
                values.into_iter().enumerate().map(|(i, v)| vec![Resp::Integer((request.offset + i as u64) as i64), v]).collect()
            }
            "hash" | "set" | "zset" => {
                let command = match kind.as_str() { "hash" => "HSCAN", "set" => "SSCAN", _ => "ZSCAN" };
                // COUNT is advisory. Smaller counts reduce the chance of a
                // compact-encoded collection returning an oversized batch.
                let (cursor, values) = scan_reply(session.command(&[command, &request.table, &request.offset.to_string(), "COUNT", &request.limit.min(10).to_string()]).await?)?;
                let width = if kind == "set" { 1 } else { 2 };
                ensure!(values.len() % width == 0, "Invalid Redis collection scan reply");
                ensure!(values.len() / width <= request.limit as usize,
                    "Redis advisory SCAN batch exceeds page size; use HSCAN/SSCAN/ZSCAN in QueryConsole (no entries were silently skipped)");
                if cursor != 0 { next = Some(cursor); }
                let mut rows = Vec::new(); let mut iter = values.into_iter();
                while let Some(v) = iter.next() {
                    let mut row = vec![v]; if width == 2 { row.push(iter.next().unwrap()); } rows.push(row);
                }
                rows
            }
            _ => return Err(anyhow!("Redis key is missing or has an unsupported browse type")),
        };
        Ok(page(value_columns(&kind), rows, request.offset, request.limit, next))
    }).await
}
pub async fn execute_read_only(
    profile: &SourceProfile,
    password: &str,
    request: &QueryRequest,
) -> Result<QueryResult> {
    let command = parse_command(&request.sql)?;
    ensure!(
        (1..=200).contains(&request.limit),
        "Redis row limit must be 1 through 200"
    );
    let profile = resolved(profile)?;
    let started = Instant::now();
    bounded_with(profile.options.query_timeout_seconds, async {
        let mut session = Session::connect(&profile, password).await?;
        let args: Vec<_> = command.iter().map(String::as_str).collect();
        let reply = session.command(&args).await?;
        let mut warnings = vec!["Native Redis RESP2 reader: submit JSON arrays of strings, not SQL. Use a read-only ACL. Binary cells are hexadecimal; SCAN cursors are opaque and scans are not snapshots. Catalog omits binary/oversized key names.".into()];
        let rows = match reply { Resp::Array(values) => values.into_iter().map(|v| vec![v]).collect(), v => vec![vec![v]] };
        let mut page = page(value_columns("value"), rows, 0, request.limit, None);
        // Console replies are previews, not a row-offset pagination API.
        page.next_offset = None;
        if page.truncated { warnings.push("Cell or page preview was truncated.".into()); }
        if page.has_more { warnings.push("Reply preview row limit reached; the command was not paginated. Redis command results do not have a reusable row offset.".into()); }
        Ok(QueryResult { page, elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64, warnings })
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> SourceProfile {
        SourceProfile {
            engine: DbEngine::Redis,
            port: 6379,
            tls: TlsMode::Disabled,
            authentication: Authentication::NoAuth,
            ..SourceProfile::default()
        }
    }
    #[test]
    fn allowlist_and_database_bounds() {
        for input in [
            r#"["GET","key"]"#,
            r#"["PING"]"#,
            r#"["SCAN","0","COUNT","20","MATCH","x*"]"#,
            r#"["ZRANGE","z","0","-1","WITHSCORES"]"#,
        ] {
            validate_read_only(input).unwrap();
        }
        for input in [
            "GET key",
            r#"["SET","key","secret"]"#,
            r#"["AUTH","secret"]"#,
            r#"["SELECT","1"]"#,
            r#"["CONFIG","GET","*"]"#,
            r#"["KEYS","*"]"#,
            r#"["EVAL","x","0"]"#,
            r#"["SCAN","0","COUNT","201"]"#,
            r#"["SCAN","0","COUNT","2","COUNT","3"]"#,
            r#"["ZRANGE","z","0","-1","STORE","dest"]"#,
            r#"["GET",42]"#,
            r#"["GET","a"] ["GET","b"]"#,
        ] {
            assert!(validate_read_only(input).is_err(), "{input}");
        }
        assert!(database("65535").is_ok());
        for db in ["-1", "65536", "db0", "", " 1"] {
            assert!(database(db).is_err());
        }
        let mut p = profile();
        p.database = Some("65536".into());
        assert!(resolved(&p).is_err());
    }
    async fn decode(mut raw: &[u8]) -> Result<Resp> {
        read_resp(&mut raw, &mut Budget { bytes: 0, nodes: 0 }, 0).await
    }
    #[tokio::test]
    async fn protocol_caps_and_sanitized_errors() {
        assert!(matches!(
            decode(b"*2\r\n$3\r\nfoo\r\n:-2\r\n").await.unwrap(),
            Resp::Array(_)
        ));
        for wire in [
            b"$8388609\r\n".as_slice(),
            b"*100001\r\n",
            b"$-2\r\n",
            b"$3\r\nfooXX",
            b"+hi\n",
            b"%1\r\n",
        ] {
            assert!(decode(wire).await.is_err());
        }
        let deep = format!("{}+x\r\n", "*1\r\n".repeat(34));
        assert!(decode(deep.as_bytes()).await.is_err());
        let error = decode(b"-ERR password-secret and private-data\r\n")
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("password-secret") && !error.contains("private-data"));
        let huge = format!("+{}\r\n", "x".repeat(KEY_CAP + 1));
        assert!(decode(huge.as_bytes()).await.is_err());
    }
    #[test]
    fn previews_preserve_binary_and_numbers() {
        let mut cut = false;
        assert_eq!(
            cell(Resp::Bytes(vec![0xff, 0]), &mut cut),
            CellValue::Binary("0xff00".into())
        );
        assert_eq!(
            cell(Resp::Integer(-1), &mut cut),
            CellValue::Number("-1".into())
        );
        assert!(
            cell(Resp::Bytes("💡".repeat(4096).into_bytes()), &mut cut)
                .display()
                .len()
                <= CELL_CAP
        );
        assert!(cut);
        let p = page(
            value_columns("hash"),
            vec![vec![Resp::Null, Resp::Integer(1)]],
            0,
            10,
            Some(923),
        );
        assert_eq!(p.next_offset, Some(923));
        assert!(p.has_more);
    }
    #[tokio::test]
    async fn unsupported_clauses_fail_before_connection() {
        let mut p = profile();
        p.port = 1;
        let req = BrowseRequest {
            database: "0".into(),
            table: "x".into(),
            where_clause: "1=1".into(),
            ..BrowseRequest::default()
        };
        assert!(
            browse(&p, "", &req)
                .await
                .unwrap_err()
                .to_string()
                .contains("SQL filters")
        );
    }
    async fn mock(
        steps: Vec<(Vec<&'static str>, &'static [u8])>,
    ) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = BufReader::new(socket);
                for (expected, reply) in steps {
                    let actual = array(
                        read_resp(&mut socket, &mut Budget { bytes: 0, nodes: 0 }, 0)
                            .await
                            .unwrap(),
                    )
                    .unwrap();
                    let actual: Vec<_> = actual
                        .into_iter()
                        .map(|r| String::from_utf8(bytes(r).unwrap()).unwrap())
                        .collect();
                    assert_eq!(actual, expected);
                    socket.write_all(reply).await.unwrap();
                    socket.flush().await.unwrap();
                }
            })
            .await
            .unwrap();
        });
        (port, task)
    }
    #[tokio::test]
    async fn owned_native_handshake_catalog_and_browse() {
        let (port, server) = mock(vec![
            (vec!["AUTH", "default", "private-password"], b"+OK\r\n"),
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["PING"], b"+PONG\r\n"),
            (
                vec!["INFO", "keyspace"],
                b"$23\r\n# Keyspace\r\ndb0:keys=1\n\r\n",
            ),
        ])
        .await;
        let mut p = profile();
        p.host = "127.0.0.1".into();
        p.port = port;
        p.authentication = Authentication::UserPassword;
        p.username = "default".into();
        let report = test_connection(&p, "private-password").await.unwrap();
        assert_eq!(report.databases, vec!["0"]);
        server.await.unwrap();
        let (port, server) = mock(vec![
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["PING"], b"+PONG\r\n"),
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (
                vec!["SCAN", "0", "COUNT", "200"],
                b"*2\r\n$1\r\n0\r\n*1\r\n$1\r\nk\r\n",
            ),
            (vec!["TYPE", "k"], b"+string\r\n"),
        ])
        .await;
        p.port = port;
        p.authentication = Authentication::NoAuth;
        assert_eq!(tables(&p, "ignored", "0").await.unwrap()[0].name, "k");
        server.await.unwrap();
        let (port, server) = mock(vec![
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["PING"], b"+PONG\r\n"),
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["TYPE", "k"], b"+string\r\n"),
            (vec!["GET", "k"], b"$3\r\nabc\r\n"),
        ])
        .await;
        p.port = port;
        let req = BrowseRequest {
            database: "0".into(),
            table: "k".into(),
            ..BrowseRequest::default()
        };
        assert_eq!(
            browse(&p, "", &req).await.unwrap().rows[0][0],
            CellValue::Text("abc".into())
        );
        server.await.unwrap();
    }
    #[tokio::test]
    async fn opaque_scan_cursor_columns_and_native_command() {
        let mut p = profile();
        p.host = "127.0.0.1".into();
        let (port, server) = mock(vec![
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["PING"], b"+PONG\r\n"),
            (vec!["SELECT", "2"], b"+OK\r\n"),
            (vec!["TYPE", "h"], b"+hash\r\n"),
            (
                vec!["HSCAN", "h", "81", "COUNT", "10"],
                b"*2\r\n$3\r\n923\r\n*2\r\n$1\r\nf\r\n$1\r\nv\r\n",
            ),
        ])
        .await;
        p.port = port;
        let request = BrowseRequest {
            database: "2".into(),
            table: "h".into(),
            offset: 81,
            limit: 100,
            ..BrowseRequest::default()
        };
        let result = browse(&p, "", &request).await.unwrap();
        assert_eq!(result.next_offset, Some(923));
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.columns[0].name, "field");
        server.await.unwrap();
        let (port, server) = mock(vec![
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["PING"], b"+PONG\r\n"),
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["TYPE", "h"], b"+hash\r\n"),
        ])
        .await;
        p.port = port;
        assert_eq!(columns(&p, "", "0", "h").await.unwrap().len(), 2);
        server.await.unwrap();
        let (port, server) = mock(vec![
            (vec!["SELECT", "0"], b"+OK\r\n"),
            (vec!["PING"], b"+PONG\r\n"),
            (vec!["MGET", "a", "b"], b"*2\r\n$1\r\nx\r\n$-1\r\n"),
        ])
        .await;
        p.port = port;
        let result = execute_read_only(
            &p,
            "",
            &QueryRequest {
                sql: r#"["MGET","a","b"]"#.into(),
                limit: 100,
            },
        )
        .await
        .unwrap();
        assert_eq!(result.page.rows[1][0], CellValue::Null);
        server.await.unwrap();
    }
    #[test]
    fn credential_and_argument_bounds() {
        let large = serde_json::to_string(&vec!["GET", &"x".repeat(KEY_CAP + 1)]).unwrap();
        assert!(validate_read_only(&large).is_err());
        let many = serde_json::to_string(&vec!["MGET"; 129]).unwrap();
        assert!(validate_read_only(&many).is_err());
        for mode in [
            TlsMode::Disabled,
            TlsMode::Required,
            TlsMode::VerifyCa,
            TlsMode::VerifyIdentity,
        ] {
            let mut p = profile();
            p.tls = mode;
            assert!(resolved(&p).is_ok());
        }
        assert_eq!(resolved(&profile()).unwrap().database, None);
    }
    #[tokio::test]
    async fn timeout_closes_owned_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut p = profile();
        p.host = "127.0.0.1".into();
        p.port = listener.local_addr().unwrap().port();
        p.options.connect_timeout_seconds = 1;
        p.options.query_timeout_seconds = 2;
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            tokio::time::timeout(
                std::time::Duration::from_secs(4),
                socket.read_to_end(&mut data),
            )
            .await
            .unwrap()
            .unwrap();
            assert!(!data.is_empty());
        });
        assert!(
            test_connection(&p, "")
                .await
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        server.await.unwrap();
    }
}
