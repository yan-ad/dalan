//! Native PostgreSQL reader. Each operation owns its connection and relay.
//! Preview limits bound retained output, not PostgreSQL protocol allocations:
//! tokio-postgres has no configurable hard packet/message allocation cap.
use crate::{
    mysql::{
        BrowseRequest, CatalogSnapshot, CellValue, ColumnInfo, ConnectionReport, DatabaseCatalog,
        FilterOperator, SortDirection, TableInfo, TablePage, bounded_with,
    },
    query::{QueryRequest, QueryResult},
    relay::{self, Relay},
    sources::{Authentication, ConnectionMode, DbEngine, SourceProfile, TlsMode},
};
use anyhow::{Result, anyhow, ensure};
use futures_util::StreamExt;
use postgres_native_tls::MakeTlsConnector;
use std::{path::Path, time::Instant};
use tokio::task::JoinHandle;
use tokio_postgres::{
    Client, Config, Row,
    config::SslMode,
    types::{FromSql, ToSql, Type},
};

const CELL_CAP: usize = 4096;
const PAGE_CAP: usize = 2 * 1024 * 1024;
const CATALOG_CAP: usize = 1000;
const OBJECT_CAP: usize = 50_000;
const COLUMN_CAP: usize = 512;

fn driver_error(error: tokio_postgres::Error) -> anyhow::Error {
    if let Some(db) = error.as_db_error() {
        let reason = match db.code().code() {
            "28P01" | "28000" => "Authentication denied; check credentials and account grants",
            "3D000" => "Selected database does not exist",
            "42501" => "Account lacks permission for this operation",
            "57014" => "Database operation cancelled or timed out",
            "25006" => "Operation rejected by read-only transaction",
            _ => "Database rejected the operation",
        };
        anyhow!("{reason} (SQLSTATE {})", db.code().code())
    } else {
        anyhow!("PostgreSQL connection/protocol failed; check transport, TLS and credentials")
    }
}
fn name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 256 && !value.contains('\0'),
        "Invalid PostgreSQL identifier"
    );
    Ok(())
}
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn qualified(schema: &str, table: &str) -> String {
    format!("{}.{}", quote(schema), quote(table))
}
// Catalog names are always two SQL-quoted identifiers, preserving dots/quotes.
// Plain schema.table is accepted for manually authored requests when unambiguous.
fn split_table(value: &str) -> Result<(String, String)> {
    fn component(input: &str) -> Result<(String, &str)> {
        if let Some(mut rest) = input.strip_prefix('"') {
            let mut out = String::new();
            loop {
                let i = rest
                    .find('"')
                    .ok_or_else(|| anyhow!("Unterminated quoted table identifier"))?;
                out.push_str(&rest[..i]);
                rest = &rest[i + 1..];
                if let Some(next) = rest.strip_prefix('"') {
                    out.push('"');
                    rest = next;
                } else {
                    name(&out)?;
                    return Ok((out, rest));
                }
            }
        }
        let i = input.find('.').unwrap_or(input.len());
        let out = &input[..i];
        name(out)?;
        ensure!(!out.contains('"'), "Invalid quoted table identifier");
        Ok((out.to_owned(), &input[i..]))
    }
    let (schema, rest) = component(value)?;
    let rest = rest
        .strip_prefix('.')
        .ok_or_else(|| anyhow!("PostgreSQL tables must be schema-qualified"))?;
    let (table, rest) = component(rest)?;
    ensure!(rest.is_empty(), "Invalid schema-qualified table identifier");
    Ok((schema, table))
}
async fn pem(path: &str) -> Result<Vec<u8>> {
    // Read at most the limit + 1; metadata alone would permit a growing file race.
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let f = std::fs::File::open(path)
            .map_err(|_| anyhow!("Cannot open TLS certificate/key file"))?;
        let mut bytes = Vec::new();
        f.take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow!("Cannot read TLS certificate/key file"))?;
        ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "TLS certificate/key file exceeds 16 MiB"
        );
        Ok(bytes)
    })
    .await
    .map_err(|_| anyhow!("TLS certificate/key loading failed"))?
}
async fn tls(profile: &SourceProfile) -> Result<MakeTlsConnector> {
    let mut builder = native_tls::TlsConnector::builder();
    builder.danger_accept_invalid_certs(profile.tls == TlsMode::Required);
    builder.danger_accept_invalid_hostnames(matches!(
        profile.tls,
        TlsMode::Required | TlsMode::VerifyCa
    ));
    if let Some(path) = &profile.ca_path {
        let bytes = pem(path).await?;
        // native-tls accepts a certificate per call; support PEM bundles explicitly.
        let text = std::str::from_utf8(&bytes).map_err(|_| anyhow!("Invalid CA PEM encoding"))?;
        let mut count = 0;
        for segment in text.split_inclusive("-----END CERTIFICATE-----") {
            if let Some(start) = segment.find("-----BEGIN CERTIFICATE-----") {
                let cert = native_tls::Certificate::from_pem(&segment.as_bytes()[start..])
                    .map_err(|_| anyhow!("Invalid CA PEM certificate"))?;
                builder.add_root_certificate(cert);
                count += 1;
            }
        }
        ensure!(count > 0, "CA file contains no PEM certificates");
    }
    match (&profile.ssl_client_cert, &profile.ssl_client_key) {
        (Some(cert), Some(key)) => {
            let identity = native_tls::Identity::from_pkcs8(&pem(cert).await?, &pem(key).await?)
                .map_err(|_| anyhow!("Invalid or unsupported TLS client identity; use a PEM certificate chain and unencrypted PKCS#8 private key"))?;
            builder.identity(identity);
        }
        (None, None) => {}
        _ => {
            return Err(anyhow!(
                "TLS client certificate and key must both be provided"
            ));
        }
    }
    Ok(MakeTlsConnector::new(
        builder
            .build()
            .map_err(|_| anyhow!("TLS configuration failed"))?,
    ))
}
struct Session {
    client: Client,
    task: JoinHandle<()>,
    _relay: Option<Relay>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Session {
    async fn connect(
        profile: &SourceProfile,
        password: &str,
        database: Option<&str>,
    ) -> Result<Self> {
        Self::connect_mode(profile, password, database, true).await
    }
    async fn connect_write(
        profile: &SourceProfile,
        password: &str,
        database: &str,
    ) -> Result<Self> {
        Self::connect_mode(profile, password, Some(database), false).await
    }
    async fn connect_mode(
        profile: &SourceProfile,
        password: &str,
        database: Option<&str>,
        read_only: bool,
    ) -> Result<Self> {
        let mut profile = profile.resolved()?;
        ensure!(
            profile.engine == DbEngine::PostgreSql,
            "This executor supports PostgreSQL only"
        );
        if let Some(database) = database {
            name(database)?;
            profile.database = Some(database.to_owned());
        }
        bounded_with(
            profile.options.connect_timeout_seconds,
            Self::connect_resolved(&profile, password, read_only),
        )
        .await
    }
    async fn connect_resolved(
        profile: &SourceProfile,
        password: &str,
        read_only: bool,
    ) -> Result<Self> {
        let mut config = Config::new();
        let username = if profile.username.is_empty() {
            "postgres"
        } else {
            &profile.username
        };
        config
            .user(username)
            .dbname(profile.database.as_deref().unwrap_or("postgres"));
        if profile.authentication != Authentication::NoAuth {
            config.password(password);
        }
        config.connect_timeout(std::time::Duration::from_secs(
            profile.options.connect_timeout_seconds,
        ));
        let relay = if let ConnectionMode::UnixSocket { path } = &profile.endpoint {
            let path = Path::new(path);
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let directory = if filename.starts_with(".s.PGSQL.") {
                path.parent()
                    .ok_or_else(|| anyhow!("Invalid PostgreSQL socket path"))?
            } else {
                path
            };
            config.host_path(directory).port(profile.port);
            None
        } else {
            let relay = relay::start(profile).await?;
            // Host remains the TLS identity, hostaddr selects the relay transport.
            config
                .host(&profile.host)
                .port(relay.as_ref().map_or(profile.port, |r| r.port));
            if let Some(r) = &relay {
                for ip in r.ips() {
                    config.hostaddr(ip);
                }
            }
            relay
        };
        config.ssl_mode(if profile.tls == TlsMode::Disabled {
            SslMode::Disable
        } else {
            SslMode::Require
        });
        let connector = tls(profile).await?;
        let (client, connection) = config.connect(connector).await.map_err(driver_error)?;
        let task = tokio::spawn(async move {
            let _ = connection.await;
        });
        let session = Self {
            client,
            task,
            _relay: relay,
        };
        session
            .client
            .batch_execute(if read_only {
                "BEGIN READ ONLY"
            } else {
                "BEGIN"
            })
            .await
            .map_err(driver_error)?;
        let timeout = format!("{}ms", profile.options.query_timeout_seconds * 1000);
        session
            .client
            .query_one(
                "SELECT pg_catalog.set_config('statement_timeout', $1, true)",
                &[&timeout],
            )
            .await
            .map_err(driver_error)?;
        Ok(session)
    }
    async fn finish(&self) -> Result<()> {
        self.client
            .batch_execute("ROLLBACK")
            .await
            .map_err(driver_error)
    }
}
async fn databases(client: &Client) -> Result<Vec<String>> {
    let rows = client.query("SELECT datname FROM pg_catalog.pg_database WHERE datallowconn AND NOT datistemplate AND pg_catalog.has_database_privilege(datname, 'CONNECT') ORDER BY datname LIMIT 1001", &[]).await.map_err(driver_error)?;
    ensure!(
        rows.len() <= CATALOG_CAP,
        "Database catalog exceeds 1000 databases"
    );
    Ok(rows.iter().map(|r| r.get(0)).collect())
}
async fn table_metadata(client: &Client) -> Result<Vec<TableInfo>> {
    let rows = client.query("SELECT n.nspname, c.relname, c.relkind::text FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE c.relkind IN ('r','p','v','m','f') AND n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname !~ '^pg_(toast|temp)' AND pg_catalog.has_schema_privilege(n.oid,'USAGE') AND pg_catalog.has_table_privilege(c.oid,'SELECT') ORDER BY n.nspname,c.relname LIMIT 50001", &[]).await.map_err(driver_error)?;
    ensure!(
        rows.len() <= OBJECT_CAP,
        "Table catalog exceeds 50000 objects"
    );
    Ok(rows
        .iter()
        .map(|r| TableInfo {
            name: qualified(r.get(0), r.get(1)),
            kind: match r.get::<_, &str>(2) {
                "v" => "VIEW",
                "m" => "MATERIALIZED VIEW",
                "f" => "FOREIGN TABLE",
                _ => "BASE TABLE",
            }
            .into(),
        })
        .collect())
}
async fn column_metadata(client: &Client, table: &str) -> Result<Vec<ColumnInfo>> {
    let (schema, table) = split_table(table)?;
    let rows = client.query("SELECT a.attname, pg_catalog.format_type(a.atttypid,a.atttypmod), NOT a.attnotnull, EXISTS (SELECT 1 FROM pg_catalog.pg_index i WHERE i.indrelid=c.oid AND i.indisprimary AND a.attnum=ANY(i.indkey)) FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class c ON c.oid=a.attrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname=$2 AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum LIMIT 513", &[&schema, &table]).await.map_err(driver_error)?;
    ensure!(rows.len() <= COLUMN_CAP, "Table exceeds 512 columns");
    ensure!(
        !rows.is_empty(),
        "Table does not exist or has no visible columns"
    );
    Ok(rows
        .iter()
        .map(|r| ColumnInfo {
            name: r.get(0),
            data_type: r.get(1),
            nullable: r.get(2),
            is_primary_key: r.get(3),
        })
        .collect())
}
pub async fn test_connection(profile: &SourceProfile, password: &str) -> Result<ConnectionReport> {
    bounded_with(profile.options.query_timeout_seconds, async {
        let s = Session::connect(profile, password, None).await?;
        let version: String = s
            .client
            .query_one("SHOW server_version", &[])
            .await
            .map_err(driver_error)?
            .get(0);
        let databases = databases(&s.client).await?;
        s.finish().await?;
        Ok(ConnectionReport {
            server_version: version,
            databases,
        })
    })
    .await
}
pub async fn tables(
    profile: &SourceProfile,
    password: &str,
    database: &str,
) -> Result<Vec<TableInfo>> {
    bounded_with(profile.options.query_timeout_seconds, async {
        let s = Session::connect(profile, password, Some(database)).await?;
        let result = table_metadata(&s.client).await?;
        s.finish().await?;
        Ok(result)
    })
    .await
}
pub async fn columns(
    profile: &SourceProfile,
    password: &str,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>> {
    bounded_with(profile.options.query_timeout_seconds, async {
        let s = Session::connect(profile, password, Some(database)).await?;
        let result = column_metadata(&s.client, table).await?;
        s.finish().await?;
        Ok(result)
    })
    .await
}
pub async fn discover_catalog(profile: &SourceProfile, password: &str) -> Result<CatalogSnapshot> {
    bounded_with(120, async {
        let report = test_connection(profile, password).await?;
        let mut databases = Vec::new();
        let mut total = 0;
        for database in report.databases {
            if !profile.visible_schema(&database) {
                continue;
            }
            let tables = tables(profile, password, &database).await?;
            total += tables.len();
            ensure!(total <= OBJECT_CAP, "Catalog exceeds 50000 objects");
            databases.push(DatabaseCatalog {
                name: database,
                tables,
            });
        }
        Ok(CatalogSnapshot { databases })
    })
    .await
}

// Native binary codecs avoid coercing all values into text. Unsupported types are
// rendered as bounded hex rather than guessing their encoding or failing a page.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::from("0x");
    for b in bytes.iter().take((CELL_CAP - 2) / 2) {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}
fn clip(mut text: String) -> String {
    if text.len() > CELL_CAP {
        let mut n = CELL_CAP;
        while !text.is_char_boundary(n) {
            n -= 1;
        }
        text.truncate(n);
    }
    text
}
fn numeric(raw: &[u8]) -> Option<String> {
    if raw.len() < 8 || !raw.len().is_multiple_of(2) {
        return None;
    }
    let word = |i| u16::from_be_bytes([raw[i], raw[i + 1]]);
    let n = word(0) as usize;
    let weight = word(2) as i16 as i32;
    let sign = word(4);
    let scale = word(6) as usize;
    if raw.len() != 8 + n * 2 {
        return None;
    }
    match sign {
        0xc000 => return Some("NaN".into()),
        0xd000 => return Some("Infinity".into()),
        0xf000 => return Some("-Infinity".into()),
        0 | 0x4000 => {}
        _ => return None,
    }
    if (0..n).any(|i| word(8 + i * 2) >= 10000) {
        return None;
    }
    let digit = |position: i32| {
        let i = weight - position;
        if i >= 0 && (i as usize) < n {
            word(8 + i as usize * 2)
        } else {
            0
        }
    };
    let mut out = if sign == 0x4000 {
        "-".to_owned()
    } else {
        String::new()
    };
    use std::fmt::Write;
    if weight < 0 {
        out.push('0');
    } else {
        for position in (0..=weight).rev() {
            if out.len() >= CELL_CAP {
                break;
            }
            if position == weight {
                let _ = write!(out, "{}", digit(position));
            } else {
                let _ = write!(out, "{:04}", digit(position));
            }
        }
    }
    if scale > 0 && out.len() < CELL_CAP {
        out.push('.');
        let start = out.len();
        for position in 1..=scale.div_ceil(4) {
            if out.len() >= CELL_CAP {
                break;
            }
            let _ = write!(out, "{:04}", digit(-(position as i32)));
        }
        out.truncate((start + scale).min(out.len()));
    }
    Some(clip(out))
}
struct Decoded(CellValue);
impl<'a> FromSql<'a> for Decoded {
    fn from_sql(
        ty: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        macro_rules! scalar {
            ($t:ty, $kind:ident) => {
                CellValue::$kind(clip(<$t>::from_sql(ty, raw)?.to_string()))
            };
        }
        let cell = match *ty {
            Type::BOOL => scalar!(bool, Text),
            Type::INT2 => scalar!(i16, Number),
            Type::INT4 => scalar!(i32, Number),
            Type::INT8 => scalar!(i64, Number),
            Type::OID => scalar!(u32, Number),
            Type::FLOAT4 => scalar!(f32, Number),
            Type::FLOAT8 => scalar!(f64, Number),
            Type::NUMERIC => numeric(raw)
                .map(CellValue::Number)
                .unwrap_or_else(|| CellValue::Binary(hex(raw))),
            Type::DATE => scalar!(chrono::NaiveDate, Temporal),
            Type::TIME => scalar!(chrono::NaiveTime, Temporal),
            Type::TIMESTAMP => scalar!(chrono::NaiveDateTime, Temporal),
            Type::TIMESTAMPTZ => scalar!(chrono::DateTime<chrono::Utc>, Temporal),
            Type::TEXT
            | Type::VARCHAR
            | Type::BPCHAR
            | Type::NAME
            | Type::UNKNOWN
            | Type::JSON
            | Type::XML => CellValue::Text(clip(
                String::from_utf8_lossy(&raw[..raw.len().min(CELL_CAP + 4)]).into_owned(),
            )),
            Type::JSONB if raw.first() == Some(&1) => CellValue::Text(clip(
                String::from_utf8_lossy(&raw[1..raw.len().min(CELL_CAP + 5)]).into_owned(),
            )),
            _ => CellValue::Binary(hex(raw)),
        };
        Ok(Self(cell))
    }
    fn from_sql_null(
        _: &Type,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(CellValue::Null))
    }
    fn accepts(_: &Type) -> bool {
        true
    }
}
fn row_cells(row: &Row) -> Result<Vec<CellValue>> {
    (0..row.len())
        .map(|i| {
            row.try_get::<_, Decoded>(i)
                .map(|v| v.0)
                .map_err(driver_error)
        })
        .collect()
}
// Clipping at a UTF-8 boundary can leave up to three bytes below the cap.
// Exact near-cap values are conservatively marked too: a preview must never
// supply a clipped original to a staged write.
fn cells_may_be_truncated(cells: &[CellValue]) -> bool {
    cells.iter().any(|c| c.display().len() >= CELL_CAP - 3)
}
#[cfg(test)]
#[test]
fn utf8_clipped_previews_are_always_marked() {
    for text in [
        "x".repeat(CELL_CAP + 1),
        "雪".repeat(CELL_CAP),
        format!("x{}", "🦀".repeat(CELL_CAP)),
    ] {
        let clipped = clip(text);
        assert!(clipped.len() <= CELL_CAP);
        assert!(cells_may_be_truncated(&[CellValue::Text(clipped)]));
    }
    for len in CELL_CAP - 3..=CELL_CAP {
        assert!(cells_may_be_truncated(&[CellValue::Text("x".repeat(len))]));
    }
    assert!(!cells_may_be_truncated(&[CellValue::Text(
        "x".repeat(CELL_CAP - 4)
    )]));
}
async fn preview(
    client: &Client,
    sql: &str,
    params: &[&(dyn ToSql + Sync)],
    columns: Vec<ColumnInfo>,
    offset: u64,
    limit: u32,
) -> Result<TablePage> {
    let stream = client
        .query_raw(sql, params.iter().copied())
        .await
        .map_err(driver_error)?;
    tokio::pin!(stream);
    let mut rows = Vec::new();
    let mut bytes = 0;
    let mut truncated = false;
    let mut has_more = false;
    while let Some(row) = stream.next().await {
        let row = row.map_err(driver_error)?;
        ensure!(row.len() <= COLUMN_CAP, "Query exceeds 512 columns");
        if rows.len() == limit as usize {
            has_more = true;
            break;
        }
        let cells = row_cells(&row)?;
        let size: usize = cells
            .iter()
            .map(|c| match c {
                CellValue::Null => 0,
                CellValue::Text(s)
                | CellValue::Number(s)
                | CellValue::Binary(s)
                | CellValue::Temporal(s) => s.len(),
            })
            .sum();
        if bytes + size > PAGE_CAP {
            has_more = true;
            truncated = true;
            break;
        }
        truncated |= cells_may_be_truncated(&cells);
        bytes += size;
        rows.push(cells);
    }
    let next_offset = if has_more {
        Some(
            offset
                .checked_add(rows.len() as u64)
                .ok_or_else(|| anyhow!("Pagination offset overflow"))?,
        )
    } else {
        None
    };
    Ok(TablePage {
        columns,
        rows,
        has_more,
        next_offset,
        offset,
        truncated,
    })
}
pub async fn browse(
    profile: &SourceProfile,
    password: &str,
    request: &BrowseRequest,
) -> Result<TablePage> {
    ensure!(
        (1..=200).contains(&request.limit),
        "Page limit must be 1 through 200"
    );
    ensure!(
        request.offset <= i64::MAX as u64,
        "Pagination offset is too large"
    );
    ensure!(
        request.where_clause.trim().is_empty() && request.order_by.trim().is_empty(),
        "PostgreSQL free-form WHERE/ORDER BY clauses are unsupported; use structured filters and sorting"
    );
    bounded_with(profile.options.query_timeout_seconds, async {
        let s = Session::connect(profile, password, Some(&request.database)).await?;
        let columns = column_metadata(&s.client, &request.table).await?;
        let (schema, table) = split_table(&request.table)?;
        let mut sql = format!("SELECT * FROM {}", qualified(&schema, &table));
        let mut params: Vec<&(dyn ToSql + Sync)> = Vec::new();
        if let Some(filter) = &request.filter {
            ensure!(
                columns.iter().any(|c| c.name == filter.column),
                "Filter column does not exist"
            );
            ensure!(
                filter.value.len() <= 4096,
                "Filter value exceeds 4096 bytes"
            );
            let col = quote(&filter.column);
            match filter.operator {
                FilterOperator::IsNull => sql.push_str(&format!(" WHERE {col} IS NULL")),
                FilterOperator::IsNotNull => sql.push_str(&format!(" WHERE {col} IS NOT NULL")),
                FilterOperator::Contains => {
                    sql.push_str(&format!(" WHERE pg_catalog.strpos({col}::text,$1::text)>0"));
                    params.push(&filter.value);
                }
                operator => {
                    let op = match operator {
                        FilterOperator::Equals => "=",
                        FilterOperator::NotEquals => "<>",
                        FilterOperator::GreaterThan => ">",
                        FilterOperator::LessThan => "<",
                        _ => unreachable!(),
                    };
                    // Text comparisons are intentional and do not interpolate metadata type strings.
                    sql.push_str(&format!(" WHERE {col}::text {op} $1::text"));
                    params.push(&filter.value);
                }
            }
        }
        if let Some(sort) = &request.sort {
            ensure!(
                columns.iter().any(|c| c.name == sort.column),
                "Sort column does not exist"
            );
            sql.push_str(&format!(
                " ORDER BY {} {}",
                quote(&sort.column),
                if sort.direction == SortDirection::Ascending {
                    "ASC"
                } else {
                    "DESC"
                }
            ));
        }
        sql.push_str(&format!(
            " LIMIT {} OFFSET {}",
            request.limit + 1,
            request.offset
        ));
        let page = preview(
            &s.client,
            &sql,
            &params,
            columns,
            request.offset,
            request.limit,
        )
        .await?;
        s.finish().await?;
        Ok(page)
    })
    .await
}
pub async fn execute_read_only(
    profile: &SourceProfile,
    password: &str,
    request: &QueryRequest,
) -> Result<QueryResult> {
    crate::query::validate_postgres_read_only(&request.sql)?;
    ensure!(
        (1..=200).contains(&request.limit),
        "Page limit must be 1 through 200"
    );
    let started = Instant::now();
    bounded_with(profile.options.query_timeout_seconds, async {
        let s = Session::connect(profile,password,None).await?;
        let statement = s.client.prepare(&request.sql).await.map_err(driver_error)?;
        ensure!(statement.columns().len() <= COLUMN_CAP, "Query exceeds 512 columns");
        let columns = statement.columns().iter().map(|c| ColumnInfo { name: c.name().into(), data_type: c.type_().name().into(), nullable: true, is_primary_key: false }).collect();
        let page = preview(&s.client,&request.sql,&[],columns,0,request.limit).await?;
        s.finish().await?;
        Ok(QueryResult { page, elapsed_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64, warnings: vec!["Read-only transactions are defense in depth; use a least-privilege account and trusted schemas. PostgreSQL wire allocations are not bounded by preview limits.".into()] })
    }).await
}

/// Lists all PostgreSQL roles with login flags and role attributes.
pub async fn list_roles(
    profile: &SourceProfile,
    password: &str,
) -> Result<Vec<crate::postgres_users::PostgresRole>> {
    let profile = profile.resolved()?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let s = Session::connect(&profile, password, None).await?;
        let sql = crate::postgres_users::list_roles_sql();
        let rows = s.client.query(sql, &[]).await.map_err(driver_error)?;
        s.finish().await?;
        Ok(crate::postgres_users::parse_roles_rows(&rows))
    })
    .await
}

/// Retrieves the structured grant lines for a specific role.
pub async fn show_role_grants(
    profile: &SourceProfile,
    password: &str,
    role: &str,
) -> Result<Vec<crate::postgres_users::PostgresGrantLine>> {
    let profile = profile.resolved()?;
    let sql = crate::postgres_users::show_grants_sql(role)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let s = Session::connect(&profile, password, None).await?;
        let rows = s.client.query(&sql, &[]).await.map_err(driver_error)?;
        s.finish().await?;
        let lines = rows
            .iter()
            .map(|r| {
                let text: String = r.get(0);
                crate::postgres_users::PostgresGrantLine::parse(text)
            })
            .collect();
        Ok(lines)
    })
    .await
}

/// Executes a role administrative DDL/DCL statement (CREATE/ALTER/DROP ROLE, GRANT, REVOKE)
/// in an explicit transaction block and commits it.
pub async fn execute_role_admin(
    profile: &SourceProfile,
    password: &str,
    database: Option<&str>,
    sql: &str,
) -> Result<()> {
    let profile = profile.resolved()?;
    let timeout = profile.options.query_timeout_seconds;
    bounded_with(timeout, async {
        let session = Session::connect_mode(&profile, password, database, false).await?;
        session.client.batch_execute(sql).await.map_err(driver_error)?;
        session.client.batch_execute("COMMIT").await.map_err(driver_error)?;
        let _ = session.finish().await;
        Ok(())
    })
    .await
}

/// Applies a parameter-only staged batch in a separate writable session.
pub async fn apply_table_changes(
    profile: &SourceProfile,
    password: &str,
    request: &crate::mutations::WriteRequest,
) -> Result<crate::mutations::WriteReport> {
    use crate::mutations::{self, Dialect, WriteFailure, WriteReport};
    mutations::validate_request(request)?;
    name(&request.database)?;
    let (schema, table_name) = split_table(&request.table)?;
    let profile = profile.resolved()?;
    let session = Session::connect_write(&profile, password, &request.database).await?;
    let timeout = profile.options.query_timeout_seconds;
    let result = bounded_with(timeout, async {
        let table = qualified(&schema, &table_name);
        let mut kind = String::new();
        for pass in 0..2 {
            let row = session.client.query_opt(
                "SELECT c.relkind::text FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname=$2",
                &[&schema, &table_name],
            ).await.map_err(driver_error)?;
            kind = row.map(|r| r.get::<_, String>(0)).unwrap_or_default();
            ensure!(matches!(kind.as_str(), "r" | "p"), "Writes require a PostgreSQL base table");
            if pass == 0 {
                session.client.batch_execute(&format!("LOCK TABLE {table} IN ROW EXCLUSIVE MODE")).await.map_err(driver_error)?;
            }
        }
        let columns = column_metadata(&session.client, &request.table).await?;
        mutations::validate_schema(&request.expected_columns, &columns)?;
        let names = columns.iter().map(|c| quote(&c.name)).collect::<Vec<_>>();
        // Ordinary inherited tables cannot borrow uniqueness from their parent.
        // Partitioned parents enforce their own composite primary key.
        let target = if kind == "r" { format!("ONLY {table}") } else { table.clone() };
        let statements = request.mutations.iter().map(|m| {
            let target = if matches!(m, mutations::RowMutation::Insert { .. }) { &table } else { &target };
            mutations::compile(m, &columns, target, &names, Dialect::Postgres)
        }).collect::<Result<Vec<_>>>()?;
        let mut report = WriteReport::default();
        for (mutation, statement) in request.mutations.iter().zip(statements) {
            let params = statement.params.iter().map(|p| p as &(dyn ToSql + Sync)).collect::<Vec<_>>();
            let affected = session.client.execute(&statement.sql, &params).await.map_err(driver_error)?;
            mutations::count(&mut report, mutation, affected)?;
        }
        Ok(report)
    }).await;
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            let _ = bounded_with(timeout, session.finish()).await;
            return Err(error);
        }
    };
    if bounded_with(timeout, async {
        session
            .client
            .batch_execute("COMMIT")
            .await
            .map_err(driver_error)
    })
    .await
    .is_err()
    {
        return Err(WriteFailure::CommitUncertain.into());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Owned trust-authenticated wire peer: no database service or credentials.
    // It implements the extended-query exchange rather than mocking Client calls.
    #[derive(Clone, Copy)]
    enum WritePeerOutcome {
        Commit,
        DropCommitAck,
        ZeroAffected,
    }

    #[derive(Default, Debug)]
    struct WriteWireTrace {
        executed: Vec<String>,
        mutation_binds: Vec<Vec<Option<Vec<u8>>>>,
    }

    fn wire_cstring<'a>(bytes: &mut &'a [u8]) -> &'a str {
        let end = bytes.iter().position(|b| *b == 0).unwrap();
        let value = std::str::from_utf8(&bytes[..end]).unwrap();
        *bytes = &bytes[end + 1..];
        value
    }

    async fn wire_message(stream: &mut tokio::net::TcpStream, tag: u8, payload: &[u8]) {
        use tokio::io::AsyncWriteExt;
        stream.write_u8(tag).await.unwrap();
        stream.write_u32((payload.len() + 4) as u32).await.unwrap();
        stream.write_all(payload).await.unwrap();
    }

    fn wire_fields(sql: &str) -> Vec<(&'static str, u32)> {
        if sql.starts_with("SELECT pg_catalog.set_config") {
            vec![("set_config", 25)]
        } else if sql.starts_with("SELECT c.relkind") {
            vec![("relkind", 25)]
        } else if sql.starts_with("SELECT a.attname") {
            vec![
                ("attname", 25),
                ("format_type", 25),
                ("?column?", 16),
                ("exists", 16),
            ]
        } else {
            assert!(
                sql.starts_with("UPDATE ONLY "),
                "Unexpected prepared SQL: {sql}"
            );
            vec![]
        }
    }

    async fn wire_description(stream: &mut tokio::net::TcpStream, sql: &str) {
        let fields = wire_fields(sql);
        if fields.is_empty() {
            wire_message(stream, b'n', &[]).await;
            return;
        }
        let mut payload = (fields.len() as u16).to_be_bytes().to_vec();
        for (name, oid) in fields {
            payload.extend_from_slice(name.as_bytes());
            payload.push(0);
            payload.extend_from_slice(&0u32.to_be_bytes()); // table OID
            payload.extend_from_slice(&0u16.to_be_bytes()); // attribute
            payload.extend_from_slice(&oid.to_be_bytes());
            payload.extend_from_slice(&(if oid == 16 { 1i16 } else { -1i16 }).to_be_bytes());
            payload.extend_from_slice(&(-1i32).to_be_bytes()); // type modifier
            payload.extend_from_slice(&0u16.to_be_bytes()); // text description
        }
        wire_message(stream, b'T', &payload).await;
    }

    async fn wire_row(stream: &mut tokio::net::TcpStream, values: &[&[u8]]) {
        let mut payload = (values.len() as u16).to_be_bytes().to_vec();
        for value in values {
            payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
            payload.extend_from_slice(value);
        }
        wire_message(stream, b'D', &payload).await;
    }

    async fn write_wire_peer(
        listener: tokio::net::TcpListener,
        outcome: WritePeerOutcome,
    ) -> WriteWireTrace {
        use std::collections::HashMap;
        use tokio::io::AsyncReadExt;
        let (mut stream, _) = listener.accept().await.unwrap();
        let length = stream.read_u32().await.unwrap();
        assert!((8..4096).contains(&length));
        let mut startup = vec![0; length as usize - 4];
        stream.read_exact(&mut startup).await.unwrap();
        assert_eq!(&startup[..4], &196608u32.to_be_bytes());
        wire_message(&mut stream, b'R', &0u32.to_be_bytes()).await; // trust auth
        wire_message(&mut stream, b'Z', b"I").await;
        let mut statements = HashMap::<String, String>::new();
        let mut portals = HashMap::<String, String>::new();
        let mut trace = WriteWireTrace::default();
        let mut in_transaction = false;
        loop {
            let tag = match stream.read_u8().await {
                Ok(tag) => tag,
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(error) => panic!("Wire read failed: {error}"),
            };
            let length = stream.read_u32().await.unwrap();
            assert!((4..65536).contains(&length));
            let mut payload = vec![0; length as usize - 4];
            stream.read_exact(&mut payload).await.unwrap();
            let mut bytes = payload.as_slice();
            match tag {
                b'Q' => {
                    let sql = wire_cstring(&mut bytes).to_owned();
                    trace.executed.push(sql.clone());
                    let command = match sql.as_str() {
                        "BEGIN" => {
                            in_transaction = true;
                            "BEGIN"
                        }
                        "COMMIT" => {
                            if matches!(outcome, WritePeerOutcome::DropCommitAck) {
                                break; // received COMMIT, deliberately no acknowledgement
                            }
                            in_transaction = false;
                            "COMMIT"
                        }
                        "ROLLBACK" => {
                            in_transaction = false;
                            "ROLLBACK"
                        }
                        "LOCK TABLE \"public\".\"fixture\" IN ROW EXCLUSIVE MODE" => "LOCK TABLE",
                        _ => panic!("Unexpected simple SQL: {sql}"),
                    };
                    wire_message(&mut stream, b'C', format!("{command}\0").as_bytes()).await;
                    wire_message(&mut stream, b'Z', if in_transaction { b"T" } else { b"I" }).await;
                }
                b'P' => {
                    let name = wire_cstring(&mut bytes).to_owned();
                    let sql = wire_cstring(&mut bytes).to_owned();
                    wire_fields(&sql); // reject unexpected SQL, even when only prepared
                    statements.insert(name, sql);
                    wire_message(&mut stream, b'1', &[]).await;
                }
                b'D' => {
                    let target = bytes[0];
                    bytes = &bytes[1..];
                    let name = wire_cstring(&mut bytes);
                    let sql = if target == b'S' {
                        &statements[name]
                    } else {
                        &portals[name]
                    };
                    if target == b'S' {
                        let count = if sql.starts_with("SELECT pg_catalog.set_config") {
                            1
                        } else if sql.starts_with("UPDATE") {
                            3
                        } else {
                            2
                        };
                        let mut params = (count as u16).to_be_bytes().to_vec();
                        for _ in 0..count {
                            params.extend_from_slice(&25u32.to_be_bytes());
                        }
                        wire_message(&mut stream, b't', &params).await;
                    }
                    wire_description(&mut stream, sql).await;
                }
                b'B' => {
                    let portal = wire_cstring(&mut bytes).to_owned();
                    let statement = wire_cstring(&mut bytes);
                    let sql = statements[statement].clone();
                    let formats = u16::from_be_bytes(bytes[..2].try_into().unwrap()) as usize;
                    bytes = &bytes[2 + formats * 2..];
                    let count = u16::from_be_bytes(bytes[..2].try_into().unwrap()) as usize;
                    bytes = &bytes[2..];
                    let mut params = Vec::new();
                    for _ in 0..count {
                        let len = i32::from_be_bytes(bytes[..4].try_into().unwrap());
                        bytes = &bytes[4..];
                        if len == -1 {
                            params.push(None);
                        } else {
                            params.push(Some(bytes[..len as usize].to_vec()));
                            bytes = &bytes[len as usize..];
                        }
                    }
                    if sql.starts_with("UPDATE") {
                        trace.mutation_binds.push(params);
                    }
                    portals.insert(portal, sql);
                    wire_message(&mut stream, b'2', &[]).await;
                }
                b'E' => {
                    let portal = wire_cstring(&mut bytes);
                    let sql = &portals[portal];
                    trace.executed.push(sql.clone());
                    let command = if sql.starts_with("SELECT pg_catalog.set_config") {
                        wire_row(&mut stream, &[b"1000"]).await;
                        "SELECT 1".to_owned()
                    } else if sql.starts_with("SELECT c.relkind") {
                        wire_row(&mut stream, &[b"r"]).await;
                        "SELECT 1".to_owned()
                    } else if sql.starts_with("SELECT a.attname") {
                        // DataRow uses the binary formats requested by tokio-postgres.
                        wire_row(&mut stream, &[b"id", b"integer", &[0], &[1]]).await;
                        wire_row(&mut stream, &[b"value", b"text", &[0], &[0]]).await;
                        "SELECT 2".to_owned()
                    } else {
                        format!(
                            "UPDATE {}",
                            if matches!(outcome, WritePeerOutcome::ZeroAffected) {
                                0
                            } else {
                                1
                            }
                        )
                    };
                    wire_message(&mut stream, b'C', format!("{command}\0").as_bytes()).await;
                }
                b'S' => {
                    wire_message(&mut stream, b'Z', if in_transaction { b"T" } else { b"I" }).await
                }
                b'C' => {
                    wire_message(&mut stream, b'3', &[]).await;
                }
                b'H' => {}
                b'X' => break,
                _ => panic!("Unexpected frontend message: {}", tag as char),
            }
        }
        // Observe the listening socket as well: reconnect/retry must not occur.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
        trace
    }

    async fn run_write_wire_fixture(
        outcome: WritePeerOutcome,
    ) -> (Result<crate::mutations::WriteReport>, WriteWireTrace) {
        use crate::mutations::{RowMutation, WriteRequest};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut profile = SourceProfile {
            engine: DbEngine::PostgreSql,
            host: "127.0.0.1".into(),
            port: listener.local_addr().unwrap().port(),
            username: "synthetic".into(),
            tls: TlsMode::Disabled,
            ..SourceProfile::default()
        };
        profile.options.connect_timeout_seconds = 1;
        profile.options.query_timeout_seconds = 1;
        let request = WriteRequest {
            database: "synthetic".into(),
            table: "public.fixture".into(),
            expected_columns: vec![
                ColumnInfo {
                    name: "id".into(),
                    data_type: "integer".into(),
                    nullable: false,
                    is_primary_key: true,
                },
                ColumnInfo {
                    name: "value".into(),
                    data_type: "text".into(),
                    nullable: false,
                    is_primary_key: false,
                },
            ],
            mutations: vec![RowMutation::Update {
                original: vec![
                    CellValue::Number("7".into()),
                    CellValue::Text("before".into()),
                ],
                changes: vec![(1, CellValue::Text("after'; COMMIT; --".into()))],
            }],
        };
        let server = tokio::spawn(write_wire_peer(listener, outcome));
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            crate::mutations::apply_table_changes(&profile, "", &request),
        )
        .await
        .unwrap();
        let trace = tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(trace.executed[0], "BEGIN");
        assert!(!trace.executed.iter().any(|sql| sql.contains("READ ONLY")));
        assert!(trace.executed[1].starts_with("SELECT pg_catalog.set_config"));
        assert!(trace.executed[2].starts_with("SELECT c.relkind"));
        assert_eq!(
            trace.executed[3],
            "LOCK TABLE \"public\".\"fixture\" IN ROW EXCLUSIVE MODE"
        );
        assert!(trace.executed[4].starts_with("SELECT c.relkind"));
        assert!(trace.executed[5].starts_with("SELECT a.attname"));
        assert!(trace.executed[6].starts_with(
            "UPDATE ONLY \"public\".\"fixture\" SET \"value\"=$1::text::pg_catalog.text WHERE "
        ));
        assert!(
            trace.executed[6].contains("\"id\" IS NOT DISTINCT FROM $2::text::pg_catalog.int4")
        );
        assert!(!trace.executed[6].contains("after"));
        assert_eq!(
            trace.mutation_binds,
            vec![vec![
                Some(b"after'; COMMIT; --".to_vec()),
                Some(b"7".to_vec()),
                Some(b"before".to_vec()),
            ]]
        );
        assert_eq!(trace.executed.len(), 8, "No mutation or transaction retry");
        (result, trace)
    }

    #[tokio::test]
    async fn postgres_write_owned_loopback_commit() {
        let (result, trace) = run_write_wire_fixture(WritePeerOutcome::Commit).await;
        assert_eq!(
            result.unwrap(),
            crate::mutations::WriteReport {
                updated: 1,
                inserted: 0,
                deleted: 0
            }
        );
        assert_eq!(trace.executed[7], "COMMIT");
    }

    #[tokio::test]
    async fn postgres_write_owned_loopback_commit_ack_drop_is_uncertain_without_retry() {
        let (result, trace) = run_write_wire_fixture(WritePeerOutcome::DropCommitAck).await;
        let error = result.unwrap_err();
        assert_eq!(
            error.downcast_ref::<crate::mutations::WriteFailure>(),
            Some(&crate::mutations::WriteFailure::CommitUncertain)
        );
        assert_eq!(trace.executed[7], "COMMIT");
        assert_eq!(
            trace
                .executed
                .iter()
                .filter(|sql| sql.as_str() == "COMMIT")
                .count(),
            1
        );
        assert!(!trace.executed.iter().any(|sql| sql == "ROLLBACK"));
    }

    #[tokio::test]
    async fn postgres_write_owned_loopback_zero_affected_rolls_back_without_commit() {
        let (result, trace) = run_write_wire_fixture(WritePeerOutcome::ZeroAffected).await;
        let error = result.unwrap_err();
        assert!(
            error
                .downcast_ref::<crate::mutations::WriteFailure>()
                .is_none()
        );
        assert!(
            error
                .to_string()
                .contains("expected exactly one affected row"),
            "{error}"
        );
        assert_eq!(trace.executed[7], "ROLLBACK");
        assert!(!trace.executed.iter().any(|sql| sql == "COMMIT"));
    }

    #[test]
    fn quoted_catalog_names_round_trip() {
        for (schema, table) in [
            ("public", "plain"),
            ("a.b", "x.y"),
            ("a\"b", "space name"),
            ("雪", "t; DROP TABLE x"),
        ] {
            assert_eq!(
                split_table(&qualified(schema, table)).unwrap(),
                (schema.into(), table.into())
            );
        }
        assert!(split_table("public.x;select").is_ok()); // quoted on use, never SQL code
        for invalid in ["plain", "a.b.c", "\"a\".\"b\";DELETE", "\"a.b"] {
            assert!(split_table(invalid).is_err());
        }
    }
    #[test]
    fn typed_values_and_caps() {
        assert_eq!(
            Decoded::from_sql(&Type::INT4, &42i32.to_be_bytes())
                .unwrap()
                .0,
            CellValue::Number("42".into())
        );
        assert_eq!(
            Decoded::from_sql_null(&Type::TEXT).unwrap().0,
            CellValue::Null
        );
        assert_eq!(
            Decoded::from_sql(&Type::BYTEA, &[0, 255]).unwrap().0,
            CellValue::Binary("0x00ff".into())
        );
        assert!(hex(&vec![255; 10000]).len() <= CELL_CAP);
        assert!(clip("雪".repeat(10000)).len() <= CELL_CAP);
        assert_eq!(
            numeric(&[0, 2, 0, 0, 0, 0, 0, 2, 0, 12, 13, 72]),
            Some("12.34".into())
        );
    }
    #[tokio::test]
    async fn cancelling_connect_closes_owned_transport() {
        use tokio::{io::AsyncReadExt, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let profile = SourceProfile {
            engine: DbEngine::PostgreSql,
            host: "127.0.0.1".into(),
            port,
            username: "synthetic".into(),
            tls: TlsMode::Disabled,
            ..SourceProfile::default()
        };
        let operation = tokio::spawn(async move { test_connection(&profile, "synthetic").await });
        let (mut stream, _) = listener.accept().await.unwrap();
        let length = stream.read_u32().await.unwrap();
        let mut payload = vec![0; length as usize - 4];
        stream.read_exact(&mut payload).await.unwrap();
        operation.abort();
        let _ = operation.await;
        let mut byte = [0; 1];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn owned_loopback_sends_postgres_startup() {
        use tokio::{io::AsyncReadExt, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let length = stream.read_u32().await.unwrap();
            assert!((8..4096).contains(&length));
            let version = stream.read_u32().await.unwrap();
            assert_eq!(version, 196608);
            let mut payload = vec![0; length as usize - 8];
            stream.read_exact(&mut payload).await.unwrap();
            assert!(payload.windows(9).any(|p| p == b"database\0"));
        });
        let profile = SourceProfile {
            engine: DbEngine::PostgreSql,
            host: "127.0.0.1".into(),
            port,
            username: "synthetic".into(),
            tls: TlsMode::Disabled,
            ..SourceProfile::default()
        };
        let error = test_connection(&profile, "synthetic-password")
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("synthetic-password"));
        server.await.unwrap();
    }
}
