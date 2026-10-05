//! Restricted MySQL/MariaDB reader. No arbitrary SQL or write API is exposed.
use crate::{
    relay::{self, Relay},
    sources::{SourceProfile, TlsMode, identifier},
};
use anyhow::{Result, anyhow, ensure};
use mysql_async::{Conn, OptsBuilder, SslOpts, Value, prelude::Queryable};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const CELL_CAP: usize = 4096;
// Preview payload bytes, not a hard bound on total allocations. Native packets and
// transient decoding allocations are separately bounded by the protocol cap.
const PAGE_CAP: usize = 2 * 1024 * 1024;
const PACKET_CAP: usize = 8 * 1024 * 1024;
const CATALOG_CAP: usize = 1000;
const TOTAL_OBJECT_CAP: usize = 50_000;
pub(crate) const COLUMN_CAP: usize = 512;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionReport {
    pub server_version: String,
    pub databases: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSort {
    pub column: String,
    pub direction: SortDirection,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub kind: String,
}
/// Complete metadata-only discovery result. Failures never return a partial catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSnapshot {
    pub databases: Vec<DatabaseCatalog>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseCatalog {
    pub name: String,
    pub tables: Vec<TableInfo>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowseRequest {
    pub database: String,
    pub table: String,
    pub filter: Option<TableFilter>,
    #[serde(default)]
    pub sort: Option<TableSort>,
    pub offset: u64,
    pub limit: u32,
}
impl Default for BrowseRequest {
    fn default() -> Self {
        Self {
            database: String::new(),
            table: String::new(),
            filter: None,
            sort: None,
            offset: 0,
            limit: 100,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableFilter {
    pub column: String,
    pub operator: FilterOperator,
    pub value: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOperator {
    Contains,
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    IsNull,
    IsNotNull,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePage {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<CellValue>>,
    pub has_more: bool,
    /// Next unread row, including when the preview byte budget ends a page early.
    pub next_offset: Option<u64>,
    pub offset: u64,
    pub truncated: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellValue {
    Null,
    Text(String),
    Binary(String),
    Number(String),
    Temporal(String),
}
impl CellValue {
    pub fn display(&self) -> String {
        match self {
            Self::Null => "NULL".into(),
            Self::Text(v) | Self::Binary(v) | Self::Number(v) | Self::Temporal(v) => v.clone(),
        }
    }
}
pub(crate) fn driver_error(e: mysql_async::Error) -> anyhow::Error {
    use mysql_async::{DriverError as D, Error as E, IoError};
    // Never retain the original error as a source: server messages, packets,
    // rows, URLs and OS error text can contain credentials or bound values.
    match e {
        E::Server(e) => {
            let reason = match e.code {
                1045 => "Authentication denied; check password and account grants",
                1044 => "Account cannot access the selected database",
                1049 => "Selected database does not exist",
                1130 => "Server does not permit this client host; check account host grants",
                1251 => "Server requires an unsupported authentication protocol",
                3159 => "Server requires secure transport; enable verified TLS",
                1820 => {
                    "Account password is expired; change it using an administrator-approved client"
                }
                _ => "Database rejected the operation",
            };
            anyhow!("{reason} (code {})", e.code)
        }
        E::Io(IoError::Io(e)) => anyhow!("Database transport/TLS I/O failed ({:?})", e.kind()),
        E::Io(IoError::Tls(e)) => {
            let reason = match e {
                mysql_async::TlsError::InvalidDnsName(_) => "invalid server identity",
                mysql_async::TlsError::Pem(_) => "invalid certificate or key PEM encoding",
                mysql_async::TlsError::VerifierBuilderError(_) => {
                    "invalid certificate trust configuration"
                }
                mysql_async::TlsError::Tls(_) => {
                    "TLS configuration or certificate verification failed"
                }
            };
            anyhow!("Database TLS failed: {reason}; check trust and server identity")
        }
        E::Driver(e) => anyhow!(
            "{}",
            match e {
                D::ConnectionClosed => "Database server closed the connection during the operation",
                D::CantParseServerVersion { .. } =>
                    "Database handshake contains an unsupported server version",
                D::UnknownAuthPlugin { .. } =>
                    "Database requested an unsupported authentication plugin",
                D::NoClientSslFlagFromServer =>
                    "Database server does not advertise TLS support; verified TLS cannot connect",
                D::NoKeyFound => "Database TLS client identity contains no usable private key",
                D::CleartextPluginDisabled =>
                    "Database requested cleartext authentication, which is disabled; no insecure fallback was attempted",
                D::MysqlOldPasswordDisabled =>
                    "Database requested legacy mysql_old_password authentication, which is disabled",
                D::UnexpectedPacket { .. } => "Database protocol failed: unexpected packet",
                D::PacketOutOfOrder => "Database protocol failed: packet out of order",
                D::PacketTooLarge => "Database protocol packet exceeds the configured limit",
                _ => "Database driver failed during the operation",
            }
        ),
        E::Url(_) => anyhow!("Invalid database connection options"),
        E::Other(_) => anyhow!(
            "Database connection or protocol failed (check TLS, credentials, and transport)"
        ),
    }
}
pub(crate) async fn bounded<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(Duration::from_secs(20), future)
        .await
        .map_err(|_| anyhow!("Database operation timed out after 20 seconds"))?
}
pub(crate) struct Session {
    conn: Option<Conn>,
    relay: Option<Relay>,
}
impl Session {
    pub(crate) async fn connect(profile: &SourceProfile, password: &str) -> Result<Self> {
        profile.validate()?;
        let relay = relay::start(profile).await?;
        let mut opts = OptsBuilder::default()
            .ip_or_hostname(&profile.host)
            .tcp_port(profile.port)
            .user(Some(&profile.username))
            .pass(Some(password))
            .db_name(profile.database.clone())
            .prefer_socket(false)
            .max_allowed_packet(Some(PACKET_CAP))
            // Disabled means no TLS request, independent of dependency defaults.
            .ssl_opts(None);
        if let Some(r) = &relay {
            opts = opts.tcp_port(r.port).resolved_ips(Some(r.ips()));
        }
        if profile.tls == TlsMode::VerifyIdentity {
            let mut ssl = SslOpts::default();
            if let Some(path) = &profile.ca_path {
                ssl = ssl.with_root_certs(vec![std::path::PathBuf::from(path).into()]);
            }
            opts = opts.ssl_opts(ssl);
        }
        let conn = match Conn::new(opts).await {
            Ok(conn) => conn,
            Err(error) => {
                // A server rejection is authoritative even if it then closes
                // the transport. Only enrich errors caused by broken forwarding.
                if !matches!(error, mysql_async::Error::Server(_))
                    && let Some(r) = &relay
                {
                    r.check_failure()?;
                }
                return Err(driver_error(error));
            }
        };
        if let Some(r) = &relay {
            r.check()?;
        }
        Ok(Self {
            conn: Some(conn),
            relay,
        })
    }
    pub(crate) fn conn(&mut self) -> &mut Conn {
        self.conn.as_mut().expect("session owns its connection")
    }
    pub(crate) fn close_transport(&mut self) {
        self.relay.take();
    }
    async fn finish(mut self) -> Result<()> {
        self.close_transport();
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.close_transport();
        // Conn's default Drop drains pending results in a detached task. Instead,
        // disconnect marks it disconnected before awaiting I/O, so cancellation
        // closes the native socket without draining untrusted metadata/results.
        if let Some(conn) = self.conn.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(1), conn.disconnect()).await;
            });
        }
    }
}
pub async fn test_connection(profile: &SourceProfile, password: &str) -> Result<ConnectionReport> {
    bounded(async {
        let mut s = Session::connect(profile, password).await?;
        let version_row: mysql_async::Row = s
            .conn()
            .query_first("SELECT VERSION()")
            .await
            .map_err(driver_error)?
            .ok_or_else(|| anyhow!("Server did not return its version"))?;
        let (server_version,): (String,) = mysql_async::from_row_opt(version_row)
            .map_err(|_| anyhow!("Invalid server version row"))?;
        let databases = database_names(s.conn(), profile.database.as_deref()).await?;
        s.finish().await?;
        Ok(ConnectionReport {
            server_version,
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
    bounded(async {
        identifier(database)?;
        let mut s = Session::connect(profile, password).await?;
        let tables = table_names(s.conn(), database).await?;
        s.finish().await?;
        Ok(tables)
    })
    .await
}

fn push_database(databases: &mut Vec<String>, name: String) -> Result<()> {
    ensure!(
        databases.len() < CATALOG_CAP,
        "Too many databases (maximum 1000)"
    );
    identifier(&name)?;
    databases.push(name);
    Ok(())
}
async fn database_names(conn: &mut Conn, selected: Option<&str>) -> Result<Vec<String>> {
    let mut databases = Vec::new();
    if let Some(name) = selected {
        push_database(&mut databases, name.to_owned())?;
    } else {
        let mut result = conn
            .query_iter("SHOW DATABASES")
            .await
            .map_err(driver_error)?;
        while let Some(row) = result.next().await.map_err(driver_error)? {
            let (name,): (String,) = mysql_async::from_row_opt(row)
                .map_err(|_| anyhow!("Invalid database metadata row"))?;
            push_database(&mut databases, name)?;
        }
    }
    Ok(databases)
}
fn push_table(tables: &mut Vec<TableInfo>, name: String, kind: String) -> Result<()> {
    ensure!(tables.len() < CATALOG_CAP, "Too many tables (maximum 1000)");
    identifier(&name)?;
    ensure!(
        matches!(
            kind.as_str(),
            "BASE TABLE" | "VIEW" | "SYSTEM VIEW" | "SEQUENCE"
        ),
        "Invalid table type metadata"
    );
    tables.push(TableInfo { name, kind });
    Ok(())
}
async fn table_names(conn: &mut Conn, database: &str) -> Result<Vec<TableInfo>> {
    identifier(database)?;
    let mut result = conn.exec_iter(
        "SELECT TABLE_NAME,TABLE_TYPE FROM information_schema.TABLES WHERE TABLE_SCHEMA=? ORDER BY TABLE_NAME LIMIT 1001",
        (database,),
    ).await.map_err(driver_error)?;
    let mut tables = Vec::new();
    while let Some(row) = result.next().await.map_err(driver_error)? {
        let (name, kind): (String, String) =
            mysql_async::from_row_opt(row).map_err(|_| anyhow!("Invalid table metadata row"))?;
        push_table(&mut tables, name, kind)?;
    }
    Ok(tables)
}
fn add_object_count(total: &mut usize, count: usize) -> Result<()> {
    ensure!(
        count <= TOTAL_OBJECT_CAP.saturating_sub(*total),
        "Too many catalog objects (maximum 50000)"
    );
    *total += count;
    Ok(())
}
/// Discovers all visible schemas, or only the explicitly configured database.
/// Uses one owned native-protocol session serially; never reads table/view data.
/// Caps: 1000 schemas, 1000 objects/schema, 50000 objects total, 8 MiB packets.
/// Each query/connect is limited to 20 seconds; the entire snapshot to 120 seconds.
/// Any error or cancellation discards the whole snapshot and closes the session.
pub async fn discover_catalog(profile: &SourceProfile, password: &str) -> Result<CatalogSnapshot> {
    tokio::time::timeout(Duration::from_secs(120), async {
        let mut session = bounded(Session::connect(profile, password)).await?;
        let names = bounded(database_names(session.conn(), profile.database.as_deref())).await?;
        let mut databases = Vec::with_capacity(names.len());
        let mut total = 0;
        for name in names {
            let tables = bounded(table_names(session.conn(), &name)).await?;
            add_object_count(&mut total, tables.len())?;
            databases.push(DatabaseCatalog { name, tables });
        }
        session.finish().await?;
        Ok(CatalogSnapshot { databases })
    })
    .await
    .map_err(|_| anyhow!("Catalog discovery timed out after 120 seconds"))?
}

async fn metadata(conn: &mut Conn, database: &str, table: &str) -> Result<Vec<ColumnInfo>> {
    let kind_row: Option<mysql_async::Row> = conn.exec_first(
        "SELECT TABLE_TYPE FROM information_schema.TABLES WHERE TABLE_SCHEMA=? AND TABLE_NAME=? LIMIT 1",
        (database, table),
    ).await.map_err(driver_error)?;
    let kind = kind_row
        .map(mysql_async::from_row_opt::<(String,)>)
        .transpose()
        .map_err(|_| anyhow!("Invalid table type metadata row"))?;
    ensure!(
        kind.as_ref().map(|(kind,)| kind.as_str()) == Some("BASE TABLE"),
        "Only actual BASE TABLE objects can be inspected or browsed (views are not executed)"
    );
    let mut result = conn.exec_iter(
        "SELECT COLUMN_NAME,DATA_TYPE,IS_NULLABLE,COLUMN_KEY FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION LIMIT 513",
        (database, table),
    ).await.map_err(driver_error)?;
    let mut columns = Vec::new();
    while let Some(row) = result.next().await.map_err(driver_error)? {
        ensure!(columns.len() < COLUMN_CAP, "Too many columns (maximum 512)");
        let (name, data_type, nullable, key): (String, String, String, String) =
            mysql_async::from_row_opt(row).map_err(|_| anyhow!("Invalid column metadata row"))?;
        columns.push(ColumnInfo {
            name,
            data_type,
            nullable: nullable == "YES",
            is_primary_key: key == "PRI",
        });
    }
    ensure!(!columns.is_empty(), "Table has no visible columns");
    Ok(columns)
}
pub async fn columns(
    profile: &SourceProfile,
    password: &str,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>> {
    bounded(async {
        identifier(database)?;
        identifier(table)?;
        let mut s = Session::connect(profile, password).await?;
        let result = metadata(s.conn(), database, table).await?;
        s.finish().await?;
        Ok(result)
    })
    .await
}
fn quote(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}
fn like(value: &str) -> String {
    format!(
        "%{}%",
        value
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_")
    )
}
fn select(request: &BrowseRequest, columns: &[ColumnInfo]) -> Result<(String, Vec<Value>)> {
    identifier(&request.database)?;
    identifier(&request.table)?;
    ensure!(
        (1..=200).contains(&request.limit),
        "Page limit must be 1 through 200"
    );
    let mut sql = format!(
        "SELECT {} FROM {}.{}",
        columns
            .iter()
            .map(|c| quote(&c.name))
            .collect::<Vec<_>>()
            .join(","),
        quote(&request.database),
        quote(&request.table)
    );
    let mut params = Vec::new();
    if let Some(f) = &request.filter {
        ensure!(
            columns.iter().any(|c| c.name == f.column),
            "Filter column is not in table metadata"
        );
        ensure!(f.value.len() <= 65536, "Filter value is too long");
        let (operator, value) = match f.operator {
            FilterOperator::Contains => ("LIKE ? ESCAPE '!'", Some(like(&f.value))),
            FilterOperator::Equals => ("= ?", Some(f.value.clone())),
            FilterOperator::NotEquals => ("<> ?", Some(f.value.clone())),
            FilterOperator::GreaterThan => ("> ?", Some(f.value.clone())),
            FilterOperator::LessThan => ("< ?", Some(f.value.clone())),
            FilterOperator::IsNull => ("IS NULL", None),
            FilterOperator::IsNotNull => ("IS NOT NULL", None),
        };
        sql.push_str(&format!(" WHERE {} {operator}", quote(&f.column)));
        if let Some(value) = value {
            params.push(Value::from(value));
        }
    }
    let mut order = Vec::new();
    if let Some(sort) = &request.sort {
        ensure!(
            columns.iter().any(|c| c.name == sort.column),
            "Sort column is not in table metadata"
        );
        let direction = match sort.direction {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        };
        order.push(format!("{} {direction}", quote(&sort.column)));
    }
    order.extend(
        columns
            .iter()
            .filter(|c| {
                c.is_primary_key && request.sort.as_ref().is_none_or(|s| s.column != c.name)
            })
            .map(|c| quote(&c.name)),
    );
    // Primary keys break sort ties in metadata order (ascending). Without a
    // primary key, equal sort values do not guarantee stable pagination.
    if !order.is_empty() {
        sql.push_str(&format!(" ORDER BY {}", order.join(",")));
    }
    sql.push_str(" LIMIT ? OFFSET ?");
    params.push(Value::UInt(u64::from(request.limit) + 1));
    params.push(Value::UInt(request.offset));
    Ok((sql, params))
}
fn decode(value: Value, column: &ColumnInfo, binary: bool, truncated: &mut bool) -> CellValue {
    match value {
        Value::NULL => CellValue::Null,
        Value::Int(v) => CellValue::Number(v.to_string()),
        Value::UInt(v) => CellValue::Number(v.to_string()),
        Value::Float(v) => CellValue::Number(v.to_string()),
        Value::Double(v) => CellValue::Number(v.to_string()),
        Value::Bytes(bytes) => {
            let numeric = matches!(
                column.data_type.as_str(),
                "decimal" | "numeric" | "newdecimal"
            );
            if (!binary || numeric || column.data_type == "json")
                && let Ok(text) = std::str::from_utf8(&bytes)
            {
                let mut end = text.len().min(CELL_CAP);
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                if end < text.len() {
                    *truncated = true;
                }
                let text = text[..end].to_owned();
                return if numeric {
                    CellValue::Number(text)
                } else {
                    CellValue::Text(text)
                };
            }
            let max = (CELL_CAP - 2) / 2;
            if bytes.len() > max {
                *truncated = true;
            }
            let mut hex = String::from("0x");
            use std::fmt::Write;
            for byte in bytes.into_iter().take(max) {
                let _ = write!(hex, "{byte:02x}");
            }
            CellValue::Binary(hex)
        }
        Value::Date(y, m, d, h, min, s, micro) => {
            let date = format!("{y:04}-{m:02}-{d:02}");
            let text = if column.data_type == "date" {
                date
            } else {
                format!("{date} {h:02}:{min:02}:{s:02}.{micro:06}")
            };
            CellValue::Temporal(text)
        }
        Value::Time(negative, days, h, m, s, micro) => CellValue::Temporal(format!(
            "{}{:02}:{m:02}:{s:02}.{micro:06}",
            if negative { "-" } else { "" },
            u64::from(days) * 24 + u64::from(h)
        )),
    }
}
pub(crate) fn decode_row(
    values: Vec<Option<Value>>,
    columns: &[ColumnInfo],
    binary: &[bool],
    truncated: &mut bool,
) -> Result<Vec<CellValue>> {
    ensure!(
        !columns.is_empty()
            && columns.len() <= COLUMN_CAP
            && values.len() == columns.len()
            && binary.len() == columns.len(),
        "Result row/metadata width does not match table columns"
    );
    values
        .into_iter()
        .zip(columns)
        .zip(binary)
        .map(|((value, column), binary)| {
            let value = value.ok_or_else(|| anyhow!("Result row contains a missing value"))?;
            Ok(decode(value, column, *binary, truncated))
        })
        .collect()
}
#[derive(Default)]
pub(crate) struct Preview {
    pub(crate) rows: Vec<Vec<CellValue>>,
    bytes: usize,
    pub(crate) truncated: bool,
    pub(crate) has_more: bool,
}
impl Preview {
    pub(crate) fn push(&mut self, values: Vec<CellValue>) -> Result<bool> {
        let size = values
            .iter()
            .map(|v| match v {
                CellValue::Null => 4,
                CellValue::Text(s)
                | CellValue::Binary(s)
                | CellValue::Number(s)
                | CellValue::Temporal(s) => s.len(),
            })
            .try_fold(0usize, |sum, len| sum.checked_add(len))
            .ok_or_else(|| anyhow!("Preview row size overflow"))?;
        ensure!(
            size <= PAGE_CAP,
            "A single row exceeds the 2 MiB preview budget"
        );
        if size > PAGE_CAP - self.bytes {
            self.truncated = true;
            self.has_more = true;
            return Ok(false);
        }
        self.bytes += size;
        self.rows.push(values);
        Ok(true)
    }
    fn next_offset(&self, offset: u64) -> Result<Option<u64>> {
        if self.has_more {
            Ok(Some(
                offset
                    .checked_add(self.rows.len() as u64)
                    .ok_or_else(|| anyhow!("Next page offset overflow"))?,
            ))
        } else {
            Ok(None)
        }
    }
}
/// Requested column ordering with ascending primary-key tie-breakers when available.
/// Without a sort, orders by primary key; without either, ordering is unspecified.
/// Equal sort values without a primary key do not guarantee stable pagination.
/// Truncated cells contain a prefix; `truncated` signals cell or page-byte truncation.
pub async fn browse(
    profile: &SourceProfile,
    password: &str,
    request: &BrowseRequest,
) -> Result<TablePage> {
    bounded(async {
        identifier(&request.database)?;
        identifier(&request.table)?;
        ensure!(
            (1..=200).contains(&request.limit),
            "Page limit must be 1 through 200"
        );
        let mut s = Session::connect(profile, password).await?;
        s.conn()
            .query_drop("START TRANSACTION READ ONLY")
            .await
            .map_err(driver_error)?;
        let columns = metadata(s.conn(), &request.database, &request.table).await?;
        let (sql, params) = select(request, &columns)?;
        let mut result = s
            .conn()
            .exec_iter(sql, params)
            .await
            .map_err(driver_error)?;
        let binary = result
            .columns_ref()
            .iter()
            .map(|c| c.character_set() == 63)
            .collect::<Vec<_>>();
        ensure!(
            binary.len() == columns.len() && binary.len() <= COLUMN_CAP,
            "Result metadata width does not match table columns"
        );
        let mut preview = Preview::default();
        while let Some(row) = result.next().await.map_err(driver_error)? {
            ensure!(
                row.len() == columns.len(),
                "Result row width does not match table columns"
            );
            if preview.rows.len() >= request.limit as usize {
                preview.has_more = true;
                break;
            }
            let values = decode_row(row.unwrap_raw(), &columns, &binary, &mut preview.truncated)?;
            if !preview.push(values)? {
                break;
            }
        }
        // Never drain unread results at the preview/lookahead cap. Closing our
        // owned transport also prevents mysql_async's Conn Drop cleanup task
        // from draining a pending result in the background. Read-only work is
        // rolled back by the server when the connection closes.
        drop(result);
        let next_offset = preview.next_offset(request.offset)?;
        s.close_transport();
        let Preview {
            rows,
            has_more,
            truncated,
            ..
        } = preview;
        Ok(TablePage {
            columns,
            rows,
            has_more,
            next_offset,
            offset: request.offset,
            truncated,
        })
    })
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_metadata_caps_and_names_are_checked_without_leaks() {
        let mut databases = Vec::new();
        for _ in 0..CATALOG_CAP {
            push_database(&mut databases, "db".into()).unwrap();
        }
        assert!(push_database(&mut databases, "overflow".into()).is_err());
        for invalid in [
            String::new(),
            " secret ".into(),
            "secret\0".into(),
            "x".repeat(65),
            "界".repeat(65),
        ] {
            let error = push_database(&mut Vec::new(), invalid.clone()).unwrap_err();
            if !invalid.is_empty() {
                assert!(!error.to_string().contains(&invalid));
            }
            assert!(push_table(&mut Vec::new(), invalid, "BASE TABLE".into()).is_err());
        }
        for name in [
            "schema with spaces",
            "double\"quote",
            "back`tick",
            &"𐍈".repeat(64),
        ] {
            push_database(&mut Vec::new(), name.into()).unwrap();
            push_table(&mut Vec::new(), name.into(), "VIEW".into()).unwrap();
        }
        let mut tables = Vec::new();
        for _ in 0..CATALOG_CAP {
            push_table(&mut tables, "t".into(), "BASE TABLE".into()).unwrap();
        }
        assert!(push_table(&mut tables, "overflow".into(), "VIEW".into()).is_err());
        assert!(push_table(&mut Vec::new(), "t".into(), "secret invalid kind".into()).is_err());
        for kind in ["VIEW", "SYSTEM VIEW", "SEQUENCE"] {
            push_table(&mut Vec::new(), "t".into(), kind.into()).unwrap();
        }
        let mut total = 0;
        for _ in 0..50 {
            add_object_count(&mut total, 1000).unwrap();
        }
        assert_eq!(total, TOTAL_OBJECT_CAP);
        add_object_count(&mut total, 0).unwrap();
        assert!(add_object_count(&mut total, 1).is_err());
        assert_eq!(total, TOTAL_OBJECT_CAP);
        assert!(add_object_count(&mut total, usize::MAX).is_err());
    }
    #[test]
    fn snapshot_serialization_has_metadata_only_contract() {
        let snapshot = CatalogSnapshot {
            databases: vec![DatabaseCatalog {
                name: "fixture".into(),
                tables: vec![TableInfo {
                    name: "contact_view".into(),
                    kind: "VIEW".into(),
                }],
            }],
        };
        let expected = serde_json::json!({"databases": [{"name": "fixture", "tables": [{"name": "contact_view", "kind": "VIEW"}]}]});
        assert_eq!(serde_json::to_value(snapshot.clone()).unwrap(), expected);
        let restored: CatalogSnapshot = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), expected);
    }
    #[test]
    fn errors_classify_without_leaking_server_or_input_values() {
        use mysql_async::{DriverError as D, Error as E, IoError, ServerError};
        const SECRET: &str = "sentinel-password-SELECT-secret-user";
        for (code, expected) in [
            (1045, "Authentication denied"),
            (1044, "cannot access"),
            (1049, "does not exist"),
            (1130, "client host"),
            (1251, "authentication protocol"),
            (3159, "secure transport"),
            (1820, "password is expired"),
            (9999, "rejected the operation"),
        ] {
            let error = driver_error(E::Server(ServerError {
                code,
                message: SECRET.into(),
                state: SECRET.into(),
            }));
            let message = format!("{error:#}");
            assert!(message.contains(expected));
            assert!(message.contains(&code.to_string()));
            assert!(!message.contains(SECRET));
            assert_eq!(error.chain().count(), 1);
        }
        for (error, expected) in [
            (
                D::UnknownAuthPlugin {
                    name: SECRET.into(),
                },
                "unsupported authentication plugin",
            ),
            (
                D::CantParseServerVersion {
                    version_string: SECRET.into(),
                },
                "server version",
            ),
            (
                D::UnexpectedPacket {
                    payload: SECRET.as_bytes().to_vec(),
                },
                "unexpected packet",
            ),
            (D::ConnectionClosed, "closed the connection"),
            (D::NoClientSslFlagFromServer, "TLS support"),
            (D::NoKeyFound, "private key"),
            (D::CleartextPluginDisabled, "no insecure fallback"),
            (D::MysqlOldPasswordDisabled, "legacy"),
            (D::PacketOutOfOrder, "out of order"),
            (D::PacketTooLarge, "configured limit"),
        ] {
            let error = driver_error(E::Driver(error));
            assert!(error.to_string().contains(expected));
            assert!(!format!("{error:#?}").contains(SECRET));
            assert_eq!(error.chain().count(), 1);
        }
        let error = driver_error(E::Io(IoError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            SECRET,
        ))));
        assert_eq!(
            error.to_string(),
            "Database transport/TLS I/O failed (ConnectionReset)"
        );
        assert!(!format!("{error:#?}").contains(SECRET));
        let error = driver_error(E::Other(Box::new(std::io::Error::other(SECRET))));
        assert!(!format!("{error:#?}").contains(SECRET));
    }
    fn col() -> ColumnInfo {
        ColumnInfo {
            name: "a`b".into(),
            data_type: "varchar".into(),
            nullable: true,
            is_primary_key: true,
        }
    }
    #[test]
    fn quotes_and_bound_filter_injection() {
        let request = BrowseRequest {
            database: "db`x".into(),
            table: "t".into(),
            filter: Some(TableFilter {
                column: "a`b".into(),
                operator: FilterOperator::Contains,
                value: "%' OR 1=1 --_!\\".into(),
            }),
            offset: 0,
            limit: 100,
            sort: None,
        };
        let (sql, params) = select(&request, &[col()]).unwrap();
        assert!(sql.contains("`db``x`.`t`"));
        assert!(sql.contains("`a``b` LIKE ? ESCAPE '!'"));
        assert!(!sql.contains("OR 1=1"));
        assert_eq!(params[0], Value::from("%!%' OR 1=1 --!_!!\\%"));
        let mut bad = request;
        bad.filter.as_mut().unwrap().column = "unknown".into();
        assert!(select(&bad, &[col()]).is_err());
    }
    #[test]
    fn null_filters_and_limits() {
        let mut r = BrowseRequest {
            database: "db".into(),
            table: "t".into(),
            filter: Some(TableFilter {
                column: "a`b".into(),
                operator: FilterOperator::IsNull,
                value: "ignored".into(),
            }),
            offset: u64::MAX,
            limit: 200,
            sort: None,
        };
        let (sql, p) = select(&r, &[col()]).unwrap();
        assert!(sql.contains("IS NULL"));
        assert_eq!(p.len(), 2);
        r.limit = 201;
        assert!(select(&r, &[col()]).is_err());
    }
    #[test]
    fn sort_quotes_identifiers_and_preserves_bound_values() {
        let mut key = col();
        key.name = "id".into();
        let mut name = col();
        name.is_primary_key = false;
        let columns = [key.clone(), name.clone()];
        for (direction, keyword) in [
            (SortDirection::Ascending, "ASC"),
            (SortDirection::Descending, "DESC"),
        ] {
            let request = BrowseRequest {
                database: "db`x".into(),
                table: "t`y".into(),
                sort: Some(TableSort {
                    column: name.name.clone(),
                    direction,
                }),
                filter: Some(TableFilter {
                    column: name.name.clone(),
                    operator: FilterOperator::Equals,
                    value: "' OR 1=1; DROP TABLE t --".into(),
                }),
                offset: 7,
                limit: 1,
            };
            let (sql, params) = select(&request, &columns).unwrap();
            assert_eq!(
                sql,
                format!(
                    "SELECT `id`,`a``b` FROM `db``x`.`t``y` WHERE `a``b` = ? ORDER BY `a``b` {keyword},`id` LIMIT ? OFFSET ?"
                )
            );
            assert_eq!(
                params,
                vec![
                    Value::from("' OR 1=1; DROP TABLE t --"),
                    Value::UInt(2),
                    Value::UInt(7)
                ]
            );
        }
    }

    #[test]
    fn sort_primary_key_ties_and_default_order() {
        let mut first = col();
        first.name = "id".into();
        let mut second = col();
        second.name = "part".into();
        let mut name = col();
        name.name = "name".into();
        name.is_primary_key = false;
        let columns = [first, second, name.clone()];
        let mut request = BrowseRequest {
            database: "db".into(),
            table: "t".into(),
            ..BrowseRequest::default()
        };
        assert!(
            select(&request, &columns)
                .unwrap()
                .0
                .contains(" ORDER BY `id`,`part` LIMIT")
        );
        for (column, expected) in [
            ("name", " ORDER BY `name` DESC,`id`,`part` LIMIT"),
            ("id", " ORDER BY `id` DESC,`part` LIMIT"),
            ("part", " ORDER BY `part` DESC,`id` LIMIT"),
        ] {
            request.sort = Some(TableSort {
                column: column.into(),
                direction: SortDirection::Descending,
            });
            assert!(select(&request, &columns).unwrap().0.contains(expected));
        }
        request.sort = None;
        assert!(
            !select(&request, &[name.clone()])
                .unwrap()
                .0
                .contains("ORDER BY")
        );
        request.sort = Some(TableSort {
            column: "name".into(),
            direction: SortDirection::Ascending,
        });
        assert!(
            select(&request, &[name])
                .unwrap()
                .0
                .contains(" ORDER BY `name` ASC LIMIT")
        );
    }

    #[test]
    fn sort_rejects_unknown_columns_and_sql_expressions() {
        let mut request = BrowseRequest {
            database: "db".into(),
            table: "t".into(),
            ..BrowseRequest::default()
        };
        for column in [
            "unknown",
            "a`b DESC",
            "a`b; DROP TABLE t --",
            "LOWER(a`b)",
            "a`b,1",
        ] {
            request.sort = Some(TableSort {
                column: column.into(),
                direction: SortDirection::Ascending,
            });
            assert_eq!(
                select(&request, &[col()]).unwrap_err().to_string(),
                "Sort column is not in table metadata"
            );
        }
    }

    #[test]
    fn sort_serde_contract_and_direction_whitelist() {
        let request: BrowseRequest = serde_json::from_value(serde_json::json!({
            "database": "db", "table": "t", "filter": null, "offset": 0, "limit": 1
        }))
        .unwrap();
        assert_eq!(request.sort, None);
        assert_eq!(BrowseRequest::default().sort, None);
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
            let sort = TableSort {
                column: "name".into(),
                direction,
            };
            assert_eq!(
                serde_json::from_str::<TableSort>(&serde_json::to_string(&sort).unwrap()).unwrap(),
                sort
            );
        }
        for direction in [
            "ASC",
            "DESC",
            "ascending",
            "Descending; DROP TABLE t",
            "DESC NULLS FIRST",
        ] {
            assert!(
                serde_json::from_value::<TableSort>(
                    serde_json::json!({"column": "name", "direction": direction})
                )
                .is_err()
            );
        }
    }

    #[test]
    fn json_is_text_even_with_binary_metadata_but_ascii_blob_is_hex() {
        let mut c = col();
        let mut truncated = false;
        c.data_type = "json".into();
        let json = br#"{"number":18446744073709551615,"text":"hello"}"#;
        assert_eq!(
            decode(Value::Bytes(json.to_vec()), &c, true, &mut truncated),
            CellValue::Text(String::from_utf8(json.to_vec()).unwrap())
        );
        c.data_type = "blob".into();
        assert_eq!(
            decode(Value::Bytes(b"hello".to_vec()), &c, true, &mut truncated),
            CellValue::Binary("0x68656c6c6f".into())
        );
        assert!(!truncated);
    }
    #[test]
    fn row_shapes_are_checked_without_panics() {
        let mut truncated = false;
        assert!(decode_row(vec![], &[col()], &[false], &mut truncated).is_err());
        assert!(decode_row(vec![Some(Value::NULL)], &[col()], &[], &mut truncated).is_err());
        assert!(decode_row(vec![None], &[col()], &[false], &mut truncated).is_err());
        let columns = vec![col(); COLUMN_CAP + 1];
        assert!(
            decode_row(
                vec![Some(Value::NULL); columns.len()],
                &columns,
                &vec![false; columns.len()],
                &mut truncated
            )
            .is_err()
        );
    }
    #[test]
    fn preview_byte_cap_continues_at_first_unread_row() {
        let mut preview = Preview::default();
        assert!(
            preview
                .push(vec![CellValue::Text("a".repeat(PAGE_CAP - 1))])
                .unwrap()
        );
        assert!(!preview.push(vec![CellValue::Text("bb".into())]).unwrap());
        assert!(preview.truncated && preview.has_more);
        assert_eq!(preview.rows.len(), 1);
        assert_eq!(preview.next_offset(100).unwrap(), Some(101));
        assert!(preview.next_offset(u64::MAX).is_err());
        let mut next = Preview::default();
        assert!(next.push(vec![CellValue::Text("bb".into())]).unwrap());
        assert_eq!(next.next_offset(101).unwrap(), None);
        next.has_more = true; // normal lookahead also advances by retained rows
        assert_eq!(next.next_offset(101).unwrap(), Some(102));
    }
    #[test]
    fn preview_rejects_oversized_first_row_instead_of_looping() {
        let mut preview = Preview::default();
        let error = preview
            .push(vec![CellValue::Text("a".repeat(PAGE_CAP + 1))])
            .unwrap_err();
        assert!(error.to_string().contains("single row exceeds"));
        assert!(preview.rows.is_empty());
        assert!(!preview.has_more);
        assert!(
            preview
                .push(vec![CellValue::Text("a".repeat(PAGE_CAP))])
                .unwrap()
        );
        assert_eq!(preview.bytes, PAGE_CAP);
    }
    #[test]
    fn lossless_native_values_and_caps() {
        let mut truncated = false;
        let c = col();
        assert_eq!(
            decode(Value::NULL, &c, false, &mut truncated),
            CellValue::Null
        );
        assert_eq!(
            decode(Value::UInt(u64::MAX), &c, false, &mut truncated).display(),
            u64::MAX.to_string()
        );
        assert_eq!(
            decode(Value::Bytes(vec![0, 255]), &c, true, &mut truncated).display(),
            "0x00ff"
        );
        let mut decimal = c.clone();
        decimal.data_type = "decimal".into();
        assert_eq!(
            decode(
                Value::Bytes(b"123.000000000000000001".to_vec()),
                &decimal,
                true,
                &mut truncated
            ),
            CellValue::Number("123.000000000000000001".into())
        );
        let v = decode(
            Value::Bytes("é".repeat(3000).into_bytes()),
            &c,
            false,
            &mut truncated,
        );
        assert!(truncated);
        assert_eq!(v.display().len(), 4096);
    }
}
