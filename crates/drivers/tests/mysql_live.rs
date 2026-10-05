//! Opt-in, read-only smoke tests. Never create/drop fixtures; console SQL is SELECT-only.
mod common;

use dalan_drivers::{
    BrowseRequest, CellValue, DbEngine, FilterOperator, SourceProfile, TableFilter, TlsMode,
    Transport, browse, columns, discover_catalog, tables, test_connection,
};

fn configured(prefix: &str, engine: DbEngine) -> anyhow::Result<(SourceProfile, String)> {
    let host = std::env::var(format!("{prefix}_HOST"))?;
    let mut profile = SourceProfile {
        host,
        engine,
        ..SourceProfile::default()
    };
    if let Ok(port) = std::env::var(format!("{prefix}_PORT")) {
        profile.port = port.parse()?;
    }
    if let Ok(user) = std::env::var(format!("{prefix}_USER")) {
        profile.username = user;
    }
    if let Ok(path) = std::env::var(format!("{prefix}_CA_PATH")) {
        profile.ca_path = Some(path);
    }
    // Disabling database TLS always requires a separate explicit opt-in.
    if std::env::var(format!("{prefix}_TLS_DISABLED")).as_deref() == Ok("1") {
        profile.tls = TlsMode::Disabled;
    }
    let password = std::env::var(format!("{prefix}_PASSWORD")).unwrap_or_default();
    Ok((profile, password))
}

async fn smoke(prefix: &str, mut profile: SourceProfile, password: String) -> anyhow::Result<()> {
    assert_eq!(profile.database, None);
    let report = test_connection(&profile, &password).await?;
    eprintln!(
        "{} server version: {}",
        profile.engine.display_name(),
        report.server_version
    );
    assert!(!report.server_version.is_empty());
    // Without a fixture database this only tests version/discovery.
    let Ok(database) = std::env::var(format!("{prefix}_DATABASE")) else {
        return Ok(());
    };
    assert!(report.databases.contains(&database));
    common::full_catalog(&profile, &password, &database).await?;
    let table = std::env::var(format!("{prefix}_TABLE")).unwrap_or_else(|_| "contact".into());
    let catalog = tables(&profile, &password, &database).await?;
    assert!(
        catalog
            .iter()
            .any(|t| t.name == table && t.kind == "BASE TABLE")
    );
    let metadata = columns(&profile, &password, &database, &table).await?;
    // Known read-only fixture contract, provisioned externally for either engine:
    // contact(id BIGINT UNSIGNED PRIMARY KEY, name VARCHAR, email VARCHAR NULL,
    // payload JSON, price DECIMAL(38,18), big_number BIGINT UNSIGNED, raw_data VARBINARY).
    // First rows: (1, 'Alice', 'alice@example.com', '{"active":true}',
    //             12.345678901234567890, 18446744073709551615),
    //             (2, 'Bob', NULL, '{"active":false}',
    //             0.000000000000000001, 9007199254740993).
    // At least three rows are required to exercise lookahead pagination.
    for name in [
        "id",
        "name",
        "email",
        "payload",
        "price",
        "big_number",
        "raw_data",
    ] {
        assert!(
            metadata.iter().any(|c| c.name == name),
            "missing fixture column {name}"
        );
    }
    assert!(
        metadata
            .iter()
            .any(|c| c.name == "id" && c.is_primary_key && !c.nullable)
    );
    assert!(metadata.iter().any(|c| c.name == "email" && c.nullable));
    let mut request = BrowseRequest {
        database: database.clone(),
        table,
        limit: 2,
        ..BrowseRequest::default()
    };
    common::sorted_pages(&profile, &password, &request).await?;
    common::queries(&profile, &password, &request).await?;
    let page = browse(&profile, &password, &request).await?;
    assert_eq!(page.rows.len(), 2);
    assert!(page.has_more);
    assert_eq!(page.next_offset, Some(2));
    let value = |row: usize, name: &str| {
        &page.rows[row][metadata.iter().position(|c| c.name == name).unwrap()]
    };
    for (row, id, name, email, price, big, active) in [
        (
            0,
            "1",
            "Alice",
            Some("alice@example.com"),
            "12.345678901234567890",
            "18446744073709551615",
            true,
        ),
        (
            1,
            "2",
            "Bob",
            None,
            "0.000000000000000001",
            "9007199254740993",
            false,
        ),
    ] {
        assert_eq!(value(row, "id"), &CellValue::Number(id.into()));
        assert_eq!(value(row, "name"), &CellValue::Text(name.into()));
        assert_eq!(
            value(row, "email"),
            &email
                .map(|s| CellValue::Text(s.into()))
                .unwrap_or(CellValue::Null)
        );
        assert_eq!(value(row, "price"), &CellValue::Number(price.into()));
        assert_eq!(value(row, "big_number"), &CellValue::Number(big.into()));
        let CellValue::Text(json) = value(row, "payload") else {
            panic!("JSON must decode as text")
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json)?,
            serde_json::json!({"active": active})
        );
    }
    let mut next_request = request.clone();
    next_request.offset = page.next_offset.unwrap();
    let next = browse(&profile, &password, &next_request).await?;
    assert_eq!(next.rows.len(), 1);
    assert!(!next.has_more);
    assert_eq!(next.next_offset, None);
    assert_ne!(next.rows[0], page.rows[0]);
    assert_ne!(next.rows[0], page.rows[1]);
    assert!(page.rows.iter().all(|r| r.len() == metadata.len()));
    let raw = metadata.iter().position(|c| c.name == "raw_data").unwrap();
    assert_eq!(page.rows[0][raw], CellValue::Binary("0x414200ff".into()));
    assert_eq!(page.rows[1][raw], CellValue::Binary("0x00".into()));
    assert!(
        catalog
            .iter()
            .any(|t| t.name == "contact_view" && t.kind == "VIEW")
    );
    let mut view = request.clone();
    view.table = "contact_view".into();
    assert!(browse(&profile, &password, &view).await.is_err());
    // All seven filter operators, literal LIKE escaping, NULL and bound injection-looking values.
    for (column, operator, literal, expected) in [
        ("name", FilterOperator::Equals, "Alice", vec!["1"]),
        ("name", FilterOperator::NotEquals, "Alice", vec!["2", "3"]),
        ("id", FilterOperator::GreaterThan, "2", vec!["3"]),
        ("id", FilterOperator::LessThan, "2", vec!["1"]),
        ("email", FilterOperator::IsNull, "", vec!["2"]),
        ("email", FilterOperator::IsNotNull, "", vec!["1", "3"]),
        ("name", FilterOperator::Contains, "%_", vec!["3"]),
        ("name", FilterOperator::Contains, "!", vec!["3"]),
        ("name", FilterOperator::Contains, "\\", vec![]),
        ("name", FilterOperator::Equals, "' OR 1=1 --", vec![]),
    ] {
        request.filter = Some(TableFilter {
            column: column.into(),
            operator,
            value: literal.into(),
        });
        let filtered = browse(&profile, &password, &request).await?;
        let id_index = filtered
            .columns
            .iter()
            .position(|c| c.name == "id")
            .unwrap();
        assert_eq!(
            filtered
                .rows
                .iter()
                .map(|r| r[id_index].display())
                .collect::<Vec<_>>(),
            expected,
            "filter {operator:?} on {column}"
        );
        assert!(!filtered.has_more);
    }
    request.filter = Some(TableFilter {
        column: "missing` OR 1=1 --".into(),
        operator: FilterOperator::Equals,
        value: "Alice".into(),
    });
    assert!(browse(&profile, &password, &request).await.is_err());
    request.filter = None;
    request.offset = u64::MAX - 1;
    let beyond = browse(&profile, &password, &request).await?;
    assert!(beyond.rows.is_empty());
    assert!(!beyond.has_more);
    assert_eq!(beyond.next_offset, None);
    profile.database = Some(database.clone());
    assert_eq!(
        test_connection(&profile, &password).await?.databases,
        vec![database]
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly configured DALAN_TEST_MYSQL_* disposable/local server"]
async fn mysql_read_only_smoke() -> anyhow::Result<()> {
    let (profile, password) = configured("DALAN_TEST_MYSQL", DbEngine::MySql)?;
    smoke("DALAN_TEST_MYSQL", profile, password).await
}
#[tokio::test]
#[ignore = "requires explicitly configured DALAN_TEST_MARIADB_* disposable/local server"]
async fn mariadb_read_only_smoke() -> anyhow::Result<()> {
    let (profile, password) = configured("DALAN_TEST_MARIADB", DbEngine::MariaDb)?;
    smoke("DALAN_TEST_MARIADB", profile, password).await
}

/// Only anonymous HTTP CONNECT is modeled here, not HTTPS or SSH. The fixture
/// target is fixed: a client cannot turn this loopback proxy into an open relay.
struct LoopbackProxy {
    port: u16,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    connections: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
impl Drop for LoopbackProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl LoopbackProxy {
    async fn start(profile: &SourceProfile, reject: bool) -> anyhow::Result<Self> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let host = profile.host.clone();
        let target_port = profile.port;
        let authority = if host.contains(':') {
            format!("[{host}]:{target_port}")
        } else {
            format!("{host}:{target_port}")
        };
        let connections = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = connections.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut client, _) = listener.accept().await?;
                let mut headers = Vec::new();
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    while !headers.ends_with(b"\r\n\r\n") {
                        anyhow::ensure!(headers.len() < 16 * 1024, "CONNECT request too large");
                        headers.push(client.read_u8().await?);
                    }
                    anyhow::Result::<()>::Ok(())
                })
                .await??;
                let headers = std::str::from_utf8(&headers)?;
                anyhow::ensure!(
                    headers.lines().next()
                        == Some(format!("CONNECT {authority} HTTP/1.1").as_str()),
                    "Unexpected CONNECT authority"
                );
                anyhow::ensure!(
                    !headers
                        .to_ascii_lowercase()
                        .contains("proxy-authorization:"),
                    "Anonymous proxy must not receive credentials"
                );
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if reject {
                    client.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n").await?;
                } else {
                    let mut upstream =
                        tokio::net::TcpStream::connect((host.as_str(), target_port)).await?;
                    client
                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                        .await?;
                    // A relay may close a socket abruptly when an operation ends.
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
                }
            }
        });
        Ok(Self {
            port,
            task,
            connections,
        })
    }
    fn transport(&self) -> Transport {
        Transport::HttpConnect {
            host: "127.0.0.1".into(),
            port: self.port,
            https: false,
        }
    }
    fn assert_healthy(&self, minimum: usize) {
        assert!(!self.task.is_finished(), "Fixture proxy failed");
        assert!(self.connections.load(std::sync::atomic::Ordering::SeqCst) >= minimum);
    }
}

async fn http_connect_smoke(prefix: &str, engine: DbEngine) -> anyhow::Result<()> {
    let (mut profile, password) = configured(prefix, engine)?;
    // This transport test explicitly isolates plaintext CONNECT from DB TLS.
    anyhow::ensure!(
        profile.tls == TlsMode::Disabled,
        "CONNECT fixture requires explicit TLS_DISABLED=1"
    );
    let proxy = LoopbackProxy::start(&profile, false).await?;
    profile.transport = proxy.transport();
    smoke(prefix, profile.clone(), password.clone()).await?;
    proxy.assert_healthy(18);
    let before = proxy.connections.load(std::sync::atomic::Ordering::SeqCst);
    let snapshot = discover_catalog(&profile, &password).await?;
    assert!(snapshot.databases.len() > 1);
    assert_eq!(
        proxy.connections.load(std::sync::atomic::Ordering::SeqCst),
        before + 1,
        "Full discovery must reuse one native session/tunnel for all schemas"
    );
    let denied = LoopbackProxy::start(&profile, true).await?;
    profile.transport = denied.transport();
    let error = test_connection(&profile, &password)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("407"),
        "CONNECT rejection must retain status, not attempt DB auth"
    );
    assert!(!error.contains(&password));
    denied.assert_healthy(1);
    Ok(())
}

async fn verified_tls_rejects_fixture(prefix: &str, engine: DbEngine) -> anyhow::Result<()> {
    let (mut profile, password) = configured(prefix, engine)?;
    profile.tls = SourceProfile::default().tls;
    assert_eq!(profile.tls, TlsMode::VerifyIdentity);
    profile.ca_path = None;
    // MySQL generates an untrusted certificate; MariaDB may have no TLS
    // provider. Neither is allowed to silently downgrade a verified session.
    assert!(test_connection(&profile, &password).await.is_err());
    Ok(())
}

#[tokio::test]
#[ignore = "requires disposable DALAN_TEST_MYSQL_* fixture and TLS_DISABLED=1"]
async fn mysql_http_connect() -> anyhow::Result<()> {
    http_connect_smoke("DALAN_TEST_MYSQL", DbEngine::MySql).await
}
#[tokio::test]
#[ignore = "requires disposable DALAN_TEST_MARIADB_* fixture and TLS_DISABLED=1"]
async fn mariadb_http_connect() -> anyhow::Result<()> {
    http_connect_smoke("DALAN_TEST_MARIADB", DbEngine::MariaDb).await
}
#[tokio::test]
#[ignore = "requires disposable untrusted/no-TLS DALAN_TEST_MYSQL_* fixture"]
async fn mysql_default_verified_tls_rejects_fixture() -> anyhow::Result<()> {
    verified_tls_rejects_fixture("DALAN_TEST_MYSQL", DbEngine::MySql).await
}
#[tokio::test]
#[ignore = "requires disposable untrusted/no-TLS DALAN_TEST_MARIADB_* fixture"]
async fn mariadb_default_verified_tls_rejects_fixture() -> anyhow::Result<()> {
    verified_tls_rejects_fixture("DALAN_TEST_MARIADB", DbEngine::MariaDb).await
}

/// scripts/test-databases provisions this account with caching_sha2_password
/// and runs this test first, without a CLI readiness login warming its cache.
#[tokio::test]
#[ignore = "requires a fresh disposable caching_sha2_password account"]
async fn mysql_uncached_sha2_without_tls() -> anyhow::Result<()> {
    let (mut profile, password) = configured("DALAN_TEST_MYSQL", DbEngine::MySql)?;
    profile.username = std::env::var("DALAN_TEST_MYSQL_UNCACHED_USER")?;
    anyhow::ensure!(
        profile.tls == TlsMode::Disabled,
        "requires explicit TLS_DISABLED=1"
    );
    assert_eq!(profile.database, None);
    let report = test_connection(&profile, &password).await?;
    assert!(report.databases.iter().any(|db| db == "dalan_fixture"));
    // The second handshake exercises the now-cached fast authentication path.
    test_connection(&profile, &password).await?;
    Ok(())
}
