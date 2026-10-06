//! Experimental native MongoDB reader (official async driver, no SQL or shell).
//! Only direct TCP is supported. Use a least-privilege read-only account and
//! trusted collections/views: validation is defense in depth, not authorization.
//! Each operation owns a pool. Cancellation drops the cursor and schedules the
//! driver's immediate asynchronous pool shutdown; server work also has maxTimeMS.
//! Preview budgets do NOT cap wire allocations: BSON documents may be 16 MiB,
//! and the official driver can decode a batch larger than the 2 MiB preview.
use crate::{
    mysql::{
        BrowseRequest, CatalogSnapshot, CellValue, ColumnInfo, ConnectionReport, DatabaseCatalog,
        TableInfo, TablePage, bounded_with,
    },
    query::{QueryRequest, QueryResult},
    sources::{Authentication, ConnectionMode, DbEngine, SourceProfile, TlsMode, Transport},
};
use anyhow::{Result, anyhow, ensure};
use futures_util::TryStreamExt;
use mongodb::{
    Client,
    bson::{Bson, Document, doc},
    options::{ClientOptions, Credential, Tls, TlsOptions},
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const CELL_CAP: usize = 4096;
const PAGE_CAP: usize = 2 * 1024 * 1024;
const CATALOG_CAP: usize = 1000;
const OBJECT_CAP: usize = 50_000;
const COMMAND_CAP: usize = 64 * 1024;
const SKIP_CAP: u64 = 1_000_000;

// Do not preserve driver errors as sources: server text, URLs and documents can
// contain secrets. Only fixed categories and numeric command codes are exposed.
fn driver_error(error: mongodb::error::Error) -> anyhow::Error {
    use mongodb::error::ErrorKind;
    match error.kind.as_ref() {
        ErrorKind::Command(e) => anyhow!("MongoDB rejected the operation (code {})", e.code),
        ErrorKind::Authentication { .. } => anyhow!("MongoDB authentication failed"),
        ErrorKind::Io(_) => anyhow!("MongoDB transport/TLS I/O failed"),
        ErrorKind::ServerSelection { .. } => {
            anyhow!("MongoDB server selection failed; check transport, TLS and availability")
        }
        ErrorKind::InvalidArgument { .. } => anyhow!("Invalid MongoDB driver options"),
        _ => anyhow!("MongoDB driver or protocol operation failed"),
    }
}
fn name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 255 && !value.chars().any(char::is_control),
        "Invalid MongoDB database or collection name"
    );
    Ok(())
}
fn resolved(profile: &SourceProfile) -> Result<SourceProfile> {
    ensure!(
        profile.engine == DbEngine::MongoDb,
        "This executor supports MongoDB only"
    );
    ensure!(
        matches!(profile.transport, Transport::Direct),
        "MongoDB supports direct transport only"
    );
    ensure!(
        !matches!(profile.endpoint, ConnectionMode::UnixSocket { .. }),
        "MongoDB Unix sockets are unsupported"
    );
    // rustls in the official driver cannot independently disable hostname checks.
    // Reject rather than silently weaken VerifyCa to unverified TLS.
    ensure!(
        profile.tls != TlsMode::VerifyCa,
        "MongoDB VerifyCa is unsupported by the official rustls backend; use VerifyIdentity or explicitly unverified Required TLS"
    );
    match (&profile.ssl_client_cert, &profile.ssl_client_key) {
        (Some(cert), Some(key)) => ensure!(
            cert == key,
            "MongoDB requires one combined PEM certificate/key file; set both client paths to the same file"
        ),
        (None, None) => {}
        _ => {
            return Err(anyhow!(
                "MongoDB requires one combined PEM certificate/key file in both client path fields"
            ));
        }
    }
    profile.resolved()
}
async fn client_options(profile: &SourceProfile, password: &str) -> Result<ClientOptions> {
    // URL contains no authentication, database names or unescaped interpolation.
    let mut uri =
        url::Url::parse("mongodb://localhost/").map_err(|_| anyhow!("Invalid MongoDB URL"))?;
    let host = if profile.host.contains(':') {
        format!("[{}]", profile.host)
    } else {
        profile.host.clone()
    };
    uri.set_host(Some(&host))
        .map_err(|_| anyhow!("Invalid MongoDB host"))?;
    uri.set_port(Some(profile.port))
        .map_err(|_| anyhow!("Invalid MongoDB port"))?;
    // Official Rust driver 3.7 explicitly rejects socketTimeoutMS. Socket reads
    // are bounded by the outer operation deadline instead, followed by immediate
    // pool shutdown on cancellation; connect/server selection have native limits.
    let mut options = ClientOptions::parse(uri.as_str())
        .await
        .map_err(driver_error)?;
    let mongo = profile.mongo_options.clone().unwrap_or_default();
    options.direct_connection = Some(mongo.direct_connection);
    options.connect_timeout = mongo
        .connect_timeout_ms
        .map(Duration::from_millis)
        .or_else(|| Some(Duration::from_secs(profile.options.connect_timeout_seconds)));
    options.server_selection_timeout = mongo
        .server_selection_timeout_ms
        .map(Duration::from_millis)
        .or(options.connect_timeout);
    options.max_pool_size = Some(2);
    options.min_pool_size = Some(0);
    options.retry_reads = Some(mongo.retry_reads.unwrap_or(false));
    options.retry_writes = Some(mongo.retry_writes.unwrap_or(false));
    if let Some(lb) = mongo.load_balanced {
        options.load_balanced = Some(lb);
    }
    if profile.authentication == Authentication::UserPassword {
        let mechanism = mongo
            .auth_mechanism
            .as_deref()
            .and_then(|m| m.parse::<mongodb::options::AuthMechanism>().ok());
        let mut cred = Credential::default();
        cred.username = Some(profile.username.clone());
        cred.password = Some(password.to_owned());
        cred.source = Some(mongo.auth_source);
        cred.mechanism = mechanism;
        options.credential = Some(cred);
    }
    options.tls = Some(if profile.tls == TlsMode::Disabled {
        Tls::Disabled
    } else {
        let mut tls = TlsOptions::default();
        tls.allow_invalid_certificates = Some(profile.tls == TlsMode::Required);
        tls.ca_file_path = profile.ca_path.as_ref().map(PathBuf::from);
        tls.cert_key_file_path = profile.ssl_client_cert.as_ref().map(PathBuf::from);
        Tls::Enabled(tls)
    });
    Ok(options)
}
struct Session(Option<Client>);
impl Session {
    async fn connect(profile: &SourceProfile, password: &str) -> Result<Self> {
        bounded_with(profile.options.connect_timeout_seconds, async {
            let client = Client::with_options(client_options(profile, password).await?)
                .map_err(driver_error)?;
            let session = Self(Some(client));
            session
                .client()
                .database(initial_database(profile))
                .run_command(doc! {"ping": 1,
                "maxTimeMS": (profile.options.connect_timeout_seconds * 1000) as i64})
                .await
                .map_err(driver_error)?;
            Ok(session)
        })
        .await
    }
    fn client(&self) -> &Client {
        self.0.as_ref().expect("live MongoDB session")
    }
    async fn finish(mut self) {
        if let Some(client) = self.0.take() {
            client.shutdown().immediate(true).await;
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(client) = self.0.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                client.shutdown().immediate(true).await;
            });
        }
    }
}
fn initial_database(profile: &SourceProfile) -> &str {
    profile.database.as_deref().unwrap_or("admin")
}

async fn database_names(client: &Client, profile: &SourceProfile) -> Result<Vec<String>> {
    if let Some(database) = &profile.database {
        return Ok(vec![database.clone()]);
    }
    // listDatabases is a single official-driver command, not a user command.
    // The server returns names in one response; the cap bounds accepted metadata,
    // not that response's native allocation.
    let mut names = client.list_database_names().await.map_err(driver_error)?;
    ensure!(
        names.len() <= CATALOG_CAP,
        "Too many MongoDB databases (maximum 1000)"
    );
    for database in &names {
        name(database)?;
    }
    names.sort();
    Ok(names)
}
async fn collection_names(client: &Client, database: &str, cap: usize) -> Result<Vec<TableInfo>> {
    let mut cursor = client
        .database(database)
        .list_collections()
        .batch_size(100)
        .await
        .map_err(driver_error)?;
    let mut tables = Vec::new();
    while let Some(spec) = cursor.try_next().await.map_err(driver_error)? {
        ensure!(
            tables.len() < cap,
            "Too many MongoDB collections (maximum 50000 total)"
        );
        name(&spec.name)?;
        tables.push(TableInfo {
            name: spec.name,
            kind: if format!("{:?}", spec.collection_type) == "View" {
                "VIEW".into()
            } else {
                "COLLECTION".into()
            },
        });
    }
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tables)
}
pub async fn test_connection(profile: &SourceProfile, password: &str) -> Result<ConnectionReport> {
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let session = Session::connect(&profile, password).await?;
        let databases = database_names(session.client(), &profile).await?;
        // Avoid buildInfo, which can require additional administrative grants.
        session.finish().await;
        Ok(ConnectionReport {
            server_version: "MongoDB (native protocol; version not queried)".into(),
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
    name(database)?;
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let session = Session::connect(&profile, password).await?;
        let result = collection_names(session.client(), database, OBJECT_CAP).await?;
        session.finish().await;
        Ok(result)
    })
    .await
}
pub async fn discover_catalog(profile: &SourceProfile, password: &str) -> Result<CatalogSnapshot> {
    let profile = resolved(profile)?;
    bounded_with(120, async {
        let session = Session::connect(&profile, password).await?;
        let names = bounded_with(
            profile.options.query_timeout_seconds,
            database_names(session.client(), &profile),
        )
        .await?;
        let mut databases = Vec::new();
        let mut remaining = OBJECT_CAP;
        for database in names {
            if !profile.visible_schema(&database) {
                continue;
            }
            let tables = bounded_with(
                profile.options.query_timeout_seconds,
                collection_names(session.client(), &database, remaining),
            )
            .await?;
            remaining -= tables.len();
            databases.push(DatabaseCatalog {
                name: database,
                tables,
            });
        }
        session.finish().await;
        Ok(CatalogSnapshot { databases })
    })
    .await
}
fn document_columns() -> Vec<ColumnInfo> {
    vec![ColumnInfo {
        name: "document".into(),
        data_type: "JSON (canonical Extended JSON)".into(),
        nullable: false,
        is_primary_key: false,
    }]
}
pub async fn columns(
    profile: &SourceProfile,
    password: &str,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>> {
    name(database)?;
    name(table)?;
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let session = Session::connect(&profile, password).await?;
        // Metadata-only existence check, never sample user documents for schema.
        let mut cursor = session
            .client()
            .database(database)
            .list_collections()
            .filter(doc! {"name": table})
            .batch_size(1)
            .await
            .map_err(driver_error)?;
        ensure!(
            cursor.try_next().await.map_err(driver_error)?.is_some(),
            "MongoDB collection does not exist"
        );
        drop(cursor);
        session.finish().await;
        Ok(document_columns())
    })
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FindCommand {
    find: String,
    #[serde(default)]
    filter: serde_json::Map<String, Value>,
    #[serde(default)]
    sort: serde_json::Map<String, Value>,
    #[serde(default)]
    projection: serde_json::Map<String, Value>,
    #[serde(default)]
    skip: u64,
    limit: Option<u32>,
}
struct ReadCommand {
    collection: String,
    filter: Document,
    sort: Document,
    projection: Document,
    skip: u64,
    limit: Option<u32>,
}
fn field(key: &str) -> Result<()> {
    ensure!(
        !key.is_empty() && key.len() <= 255 && !key.starts_with('$') && !key.contains('\0'),
        "Invalid MongoDB field name"
    );
    Ok(())
}
// Strict whitelist: no expressions, aggregation, JavaScript, geo or server-side
// functions. Values have their own traversal so dangerous operators cannot hide
// inside literal objects, arrays, logical operators or $elemMatch.
fn validate_filter(value: &Value, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    ensure!(
        depth <= 16 && *nodes <= 4096,
        "MongoDB filter exceeds complexity limits"
    );
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key.starts_with('$') {
                    ensure!(
                        matches!(
                            key.as_str(),
                            "$eq"
                                | "$ne"
                                | "$gt"
                                | "$gte"
                                | "$lt"
                                | "$lte"
                                | "$in"
                                | "$nin"
                                | "$exists"
                                | "$and"
                                | "$or"
                                | "$nor"
                                | "$not"
                                | "$elemMatch"
                                | "$regex"
                                | "$options"
                                | "$oid"
                                | "$date"
                                | "$numberInt"
                                | "$numberLong"
                                | "$numberDouble"
                                | "$numberDecimal"
                                | "$binary"
                                | "$timestamp"
                        ),
                        "Unsupported MongoDB filter operator"
                    );
                    match key.as_str() {
                        "$regex" => ensure!(
                            value.as_str().is_some_and(|v| v.len() <= 256),
                            "MongoDB regex must be a string of at most 256 bytes"
                        ),
                        "$options" => ensure!(
                            value.as_str().is_some_and(|v| v.len() <= 4
                                && v.chars().all(|c| matches!(c, 'i' | 'm' | 's' | 'x'))),
                            "Unsupported MongoDB regex options"
                        ),
                        "$exists" => {
                            ensure!(value.is_boolean(), "MongoDB $exists requires a boolean")
                        }
                        "$and" | "$or" | "$nor" => ensure!(
                            value
                                .as_array()
                                .is_some_and(|a| !a.is_empty() && a.iter().all(Value::is_object)),
                            "MongoDB logical operators require nonempty object arrays"
                        ),
                        "$in" | "$nin" => ensure!(
                            value.is_array(),
                            "MongoDB membership operators require arrays"
                        ),
                        _ => {}
                    }
                } else {
                    field(key)?;
                }
                validate_filter(value, depth + 1, nodes)?;
            }
        }
        Value::Array(values) => {
            ensure!(
                values.len() <= 256,
                "MongoDB filter array exceeds 256 elements"
            );
            for value in values {
                validate_filter(value, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn bson_document(map: serde_json::Map<String, Value>) -> Result<Document> {
    match Bson::try_from(Value::Object(map))
        .map_err(|_| anyhow!("Invalid MongoDB Extended JSON"))?
    {
        Bson::Document(document) => Ok(document),
        _ => Err(anyhow!("MongoDB command requires a document")),
    }
}
fn parse_command(text: &str) -> Result<ReadCommand> {
    ensure!(
        !text.is_empty() && text.len() <= COMMAND_CAP,
        "MongoDB command must contain 1 through 65536 bytes"
    );
    // serde_json's recursion bound protects parsing; the stricter bound above
    // protects BSON conversion. Typed deserialization rejects duplicate top-level
    // fields and unknown command fields rather than letting runCommand through.
    let command: FindCommand = serde_json::from_str(text).map_err(|_| anyhow!("Expected one JSON find command with only find, filter, sort, projection, skip and limit fields"))?;
    name(&command.find)?;
    ensure!(command.skip <= SKIP_CAP, "MongoDB skip exceeds 1000000");
    ensure!(
        command.limit.is_none_or(|limit| (1..=200).contains(&limit)),
        "MongoDB limit must be 1 through 200"
    );
    validate_filter(&Value::Object(command.filter.clone()), 0, &mut 0)?;
    ensure!(
        command.sort.len() <= 32 && command.projection.len() <= 256,
        "MongoDB sort/projection exceeds field limits"
    );
    for (key, value) in &command.sort {
        field(key)?;
        ensure!(
            matches!(value.as_i64(), Some(-1 | 1)),
            "MongoDB sort values must be 1 or -1"
        );
    }
    let mut include = false;
    let mut exclude = false;
    for (key, value) in &command.projection {
        field(key)?;
        ensure!(
            matches!(value.as_i64(), Some(0 | 1)),
            "MongoDB projection values must be 0 or 1 (expressions unsupported)"
        );
        if key != "_id" {
            include |= value.as_i64() == Some(1);
            exclude |= value.as_i64() == Some(0);
        }
    }
    ensure!(
        !(include && exclude),
        "MongoDB projection cannot mix inclusion and exclusion except _id"
    );
    Ok(ReadCommand {
        collection: command.find,
        filter: bson_document(command.filter)?,
        sort: bson_document(command.sort)?,
        projection: bson_document(command.projection)?,
        skip: command.skip,
        limit: command.limit,
    })
}
/// Validate the restricted JSON find language, not SQL or mongosh syntax.
pub fn validate_read_only(text: &str) -> Result<()> {
    parse_command(text).map(|_| ())
}

struct Preview {
    rows: Vec<Vec<CellValue>>,
    bytes: usize,
    truncated: bool,
}
impl Preview {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            bytes: 0,
            truncated: false,
        }
    }
    fn push(&mut self, document: Document) -> bool {
        // Canonical EJSON preserves ObjectId, dates, binary, decimal and numeric
        // widths. Truncated cell strings are previews, not valid round-trip JSON.
        let mut text = Bson::Document(document)
            .into_canonical_extjson()
            .to_string();
        let cut = text.len() > CELL_CAP;
        if cut {
            let mut end = CELL_CAP - '…'.len_utf8();
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text.push('…');
        }
        if self.bytes + text.len() > PAGE_CAP {
            self.truncated = true;
            return false;
        }
        self.bytes += text.len();
        self.truncated |= cut;
        self.rows.push(vec![CellValue::Text(text)]);
        true
    }
    fn page(self, offset: u64, has_more: bool) -> TablePage {
        let next_offset = has_more.then_some(offset + self.rows.len() as u64);
        TablePage {
            columns: document_columns(),
            rows: self.rows,
            has_more,
            next_offset,
            offset,
            truncated: self.truncated,
        }
    }
}
async fn find_page(
    session: &Session,
    profile: &SourceProfile,
    database: &str,
    command: ReadCommand,
    limit: u32,
) -> Result<TablePage> {
    let limit = command.limit.unwrap_or(limit).min(limit);
    let mut cursor = session
        .client()
        .database(database)
        .collection::<Document>(&command.collection)
        .find(command.filter)
        .sort(command.sort)
        .projection(command.projection)
        .skip(command.skip)
        .limit(i64::from(limit) + 1)
        .batch_size(limit + 1)
        .max_time(Duration::from_secs(profile.options.query_timeout_seconds))
        .await
        .map_err(driver_error)?;
    let mut preview = Preview::new();
    let mut has_more = false;
    while let Some(document) = cursor.try_next().await.map_err(driver_error)? {
        if preview.rows.len() >= limit as usize {
            has_more = true;
            break;
        }
        if !preview.push(document) {
            has_more = true;
            break;
        }
    }
    Ok(preview.page(command.skip, has_more))
}
pub async fn browse(
    profile: &SourceProfile,
    password: &str,
    request: &BrowseRequest,
) -> Result<TablePage> {
    // Reject unsupported clauses before any connection attempt (even empty
    // structured filter values). Mongo's native JSON is only in QueryConsole.
    ensure!(
        request.filter.is_none()
            && request.sort.is_none()
            && request.where_clause.trim().is_empty()
            && request.order_by.trim().is_empty(),
        "MongoDB browse does not support SQL filters or sorting; use a JSON find command in QueryConsole"
    );
    name(&request.database)?;
    name(&request.table)?;
    ensure!(
        (1..=200).contains(&request.limit),
        "MongoDB page size must be 1 through 200"
    );
    ensure!(request.offset <= SKIP_CAP, "MongoDB offset exceeds 1000000");
    let profile = resolved(profile)?;
    bounded_with(profile.options.query_timeout_seconds, async {
        let session = Session::connect(&profile, password).await?;
        let command = ReadCommand {
            collection: request.table.clone(),
            filter: Document::new(),
            sort: doc! {"_id": 1},
            projection: Document::new(),
            skip: request.offset,
            limit: None,
        };
        let page = find_page(
            &session,
            &profile,
            &request.database,
            command,
            request.limit,
        )
        .await?;
        session.finish().await;
        Ok(page)
    })
    .await
}
pub async fn execute_read_only(
    profile: &SourceProfile,
    password: &str,
    request: &QueryRequest,
) -> Result<QueryResult> {
    let command = parse_command(&request.sql)?;
    ensure!(
        (1..=200).contains(&request.limit),
        "MongoDB row limit must be 1 through 200"
    );
    let profile = resolved(profile)?;
    let started = Instant::now();
    bounded_with(profile.options.query_timeout_seconds, async {
        let session = Session::connect(&profile, password).await?;
        let page = find_page(&session, &profile, initial_database(&profile), command, request.limit).await?;
        session.finish().await;
        let mut warnings = vec!["Experimental MongoDB JSON find reader; authentication source is admin. Results without an explicit sort may change order between pages.".into()];
        if page.truncated { warnings.push("Document preview was truncated; truncated cells are not complete Extended JSON.".into()); }
        if page.has_more { warnings.push("More documents are available; increase skip to the next offset.".into()); }
        Ok(QueryResult { page, elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64, warnings })
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> SourceProfile {
        SourceProfile {
            engine: DbEngine::MongoDb,
            port: 27017,
            tls: TlsMode::Disabled,
            authentication: Authentication::NoAuth,
            ..SourceProfile::default()
        }
    }
    #[test]
    fn command_allowlist_and_bounds() {
        for command in [
            r#"{"find":"items"}"#,
            r#"{"find":"items","filter":{"x":{"$gte":1},"_id":{"$oid":"507f1f77bcf86cd799439011"}},"sort":{"x":-1},"projection":{"x":1,"_id":0},"limit":2}"#,
        ] {
            validate_read_only(command).unwrap();
        }
        for command in [
            "SELECT * FROM items",
            r#"{"drop":"items"}"#,
            r#"{"find":"items","maxTimeMS":0}"#,
            r#"{"find":"items","find":"other"}"#,
            r#"{"find":"items","limit":0}"#,
            r#"{"find":"items","skip":1000001}"#,
            r#"{"find":"items","sort":{"x":{"$meta":"textScore"}}}"#,
            r#"{"find":"items","projection":{"x":{"$function":{}}}}"#,
            r#"{"find":"items","projection":{"x":1,"y":0}}"#,
            r#"{"find":"items","filter":{"$where":"secret"}}"#,
            r#"{"find":"items","filter":{"x":{"$out":"target"}}}"#,
            r#"{"find":"items","filter":{"$or":[{"x":{"$function":{}}}]}}"#,
            r#"{"find":"items","filter":{"x":{"$merge":"target"}}}"#,
            r#"{"find":"items","filter":{"x":{"$accumulator":{}}}}"#,
        ] {
            assert!(validate_read_only(command).is_err(), "{command}");
        }
    }
    #[test]
    fn typed_bson_and_preview() {
        let command = parse_command(r#"{"find":"c","filter":{"_id":{"$oid":"507f1f77bcf86cd799439011"},"date":{"$date":{"$numberLong":"123"}}}}"#).unwrap();
        assert!(matches!(command.filter.get("_id"), Some(Bson::ObjectId(_))));
        assert!(matches!(
            command.filter.get("date"),
            Some(Bson::DateTime(_))
        ));
        let mut preview = Preview::new();
        assert!(preview.push(doc! {"n": 12_i64, "id": mongodb::bson::oid::ObjectId::new()}));
        let text = preview.rows[0][0].display();
        assert!(text.contains("$numberLong") && text.contains("$oid"));
        assert!(preview.push(doc! {"large": "💡".repeat(4096)}));
        assert!(preview.rows[1][0].display().len() <= CELL_CAP);
        let page = preview.page(20, true);
        assert!(page.truncated && page.has_more);
        assert_eq!(page.next_offset, Some(22));
    }
    #[test]
    fn complexity_and_page_budget() {
        let deep = format!(
            "{{\"find\":\"c\",\"filter\":{{\"x\":{}{}}}}}",
            "[".repeat(18),
            "0".to_owned() + &"]".repeat(18)
        );
        assert!(validate_read_only(&deep).is_err());
        let oversized = format!("{{\"find\":\"{}\"}}", "x".repeat(COMMAND_CAP));
        assert!(validate_read_only(&oversized).is_err());
        let mut preview = Preview::new();
        preview.bytes = PAGE_CAP - 1;
        assert!(!preview.push(doc! {"x": 1}));
        let page = preview.page(9, true);
        assert!(page.truncated);
        assert_eq!(page.next_offset, Some(9));
    }
    #[test]
    fn tls_rejection_without_reading_private_files() {
        let mut p = profile();
        p.tls = TlsMode::VerifyCa;
        assert!(resolved(&p).unwrap_err().to_string().contains("VerifyCa"));
        p.tls = TlsMode::VerifyIdentity;
        p.ssl_client_cert = Some("/not/read/cert".into());
        p.ssl_client_key = Some("/not/read/key".into());
        assert!(
            resolved(&p)
                .unwrap_err()
                .to_string()
                .contains("combined PEM")
        );
    }
    #[tokio::test]
    async fn uri_options_reach_native_driver_without_uri_credentials() {
        let p = SourceProfile {
            engine: DbEngine::MongoDb,
            endpoint: ConnectionMode::UrlOnly {
                url: "mongodb://127.0.0.1:27017/fixture?directConnection=false&authSource=accounts"
                    .into(),
            },
            username: "fixture_user".into(),
            tls: TlsMode::Disabled,
            ..SourceProfile::default()
        }
        .resolved()
        .unwrap();
        let options = client_options(&p, "fixture-secret").await.unwrap();
        assert_eq!(options.direct_connection, Some(false));
        let credential = options.credential.unwrap();
        assert_eq!(credential.source.as_deref(), Some("accounts"));
        assert_eq!(credential.username.as_deref(), Some("fixture_user"));
        assert_eq!(credential.password.as_deref(), Some("fixture-secret"));
    }
    #[tokio::test]
    async fn credentials_never_enter_uri_and_tls_modes_explicit() {
        let mut ipv6 = profile();
        ipv6.host = "::1".into();
        assert!(client_options(&ipv6, "").await.is_ok());
        for mode in [
            TlsMode::Disabled,
            TlsMode::Required,
            TlsMode::VerifyIdentity,
        ] {
            let mut p = profile();
            p.tls = mode;
            p.authentication = Authentication::UserPassword;
            p.username = "user@:/?#".into();
            let options = client_options(&p, "password@:/?#").await.unwrap();
            let credential = options.credential.unwrap();
            assert_eq!(credential.username.as_deref(), Some("user@:/?#"));
            assert_eq!(credential.password.as_deref(), Some("password@:/?#"));
            assert_eq!(credential.source.as_deref(), Some("admin"));
            match options.tls.unwrap() {
                Tls::Disabled => assert_eq!(mode, TlsMode::Disabled),
                Tls::Enabled(tls) => assert_eq!(
                    tls.allow_invalid_certificates,
                    Some(mode == TlsMode::Required)
                ),
            }
        }
    }
    #[tokio::test]
    async fn rejects_sql_browse_before_network() {
        let mut p = profile();
        p.host = "127.0.0.1".into();
        p.port = 1;
        let request = BrowseRequest {
            database: "d".into(),
            table: "c".into(),
            where_clause: "1=1".into(),
            ..BrowseRequest::default()
        };
        assert!(
            browse(&p, "", &request)
                .await
                .unwrap_err()
                .to_string()
                .contains("SQL filters")
        );
    }
    #[tokio::test]
    async fn native_mongodb_handshake_and_timeout_on_loopback() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut p = profile();
        p.host = "127.0.0.1".into();
        p.port = listener.local_addr().unwrap().port();
        p.options.connect_timeout_seconds = 1;
        p.options.query_timeout_seconds = 2;
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut header = [0_u8; 16];
            tokio::time::timeout(Duration::from_secs(3), socket.read_exact(&mut header))
                .await
                .unwrap()
                .unwrap();
            let length = i32::from_le_bytes(header[0..4].try_into().unwrap());
            let opcode = i32::from_le_bytes(header[12..16].try_into().unwrap());
            assert!((16..=65536).contains(&length));
            // Official MongoDB hello handshake: OP_QUERY or OP_MSG, not HTTP,
            // SQL, shell invocation or a Redis textual command.
            assert!(matches!(opcode, 2004 | 2013));
            let mut body = vec![0_u8; length as usize - 16];
            socket.read_exact(&mut body).await.unwrap();
            assert!(
                body.windows(8).any(|v| v == b"isMaster") || body.windows(5).any(|v| v == b"hello")
            );
            tokio::time::sleep(Duration::from_secs(2)).await;
        });
        let error = test_connection(&p, "").await.unwrap_err().to_string();
        assert!(
            error.contains("timed out") || error.contains("selection"),
            "{error}"
        );
        peer.await.unwrap();
    }
}
