//! Opt-in disposable secure fixture tests; run `scripts/test-secure-transports`.
//! Never modifies the user's OS trust store or SSH known_hosts.
mod common;

use dalan_drivers::{
    BrowseRequest, CellValue, DbEngine, SourceProfile, TlsMode, Transport, browse, columns, tables,
    test_connection,
};

fn profile(engine: DbEngine) -> anyhow::Result<SourceProfile> {
    Ok(SourceProfile {
        engine,
        host: "localhost".into(),
        port: port(engine, "PORT")?,
        username: "dalan_reader".into(),
        tls: TlsMode::VerifyIdentity,
        ca_path: Some(std::env::var("DALAN_SECURE_CA_PATH")?),
        ..SourceProfile::default()
    })
}
fn port(engine: DbEngine, suffix: &str) -> anyhow::Result<u16> {
    let name = match engine {
        DbEngine::MySql => "MYSQL",
        DbEngine::MariaDb => "MARIADB",
    };
    Ok(std::env::var(format!("DALAN_SECURE_{name}_{suffix}"))?.parse()?)
}
const PASSWORD: &str = "dalan-local-fixture-only";

async fn positive(engine: DbEngine, connect: bool) -> anyhow::Result<()> {
    let mut profile = profile(engine)?;
    if connect {
        profile.transport = Transport::HttpConnect {
            host: "localhost".into(),
            port: port(engine, "HTTP_PORT")?,
            https: false,
        };
    }
    let report = test_connection(&profile, PASSWORD).await?;
    eprintln!(
        "{} verified TLS version: {}",
        engine.display_name(),
        report.server_version
    );
    assert!(!report.server_version.is_empty());
    assert!(report.databases.iter().any(|d| d == "dalan_fixture"));
    let catalog = tables(&profile, PASSWORD, "dalan_fixture").await?;
    assert!(catalog.iter().any(|t| t.name == "contact"));
    let metadata = columns(&profile, PASSWORD, "dalan_fixture", "contact").await?;
    let request = BrowseRequest {
        database: "dalan_fixture".into(),
        table: "contact".into(),
        limit: 2,
        ..BrowseRequest::default()
    };
    common::sorted_pages(&profile, PASSWORD, &request).await?;
    let page = browse(&profile, PASSWORD, &request).await?;
    assert_eq!(page.rows.len(), 2);
    assert!(page.has_more);
    let value = |row: usize, name: &str| {
        &page.rows[row][metadata.iter().position(|c| c.name == name).unwrap()]
    };
    assert_eq!(value(0, "name"), &CellValue::Text("Alice".into()));
    assert_eq!(value(1, "email"), &CellValue::Null);
    assert_eq!(
        value(0, "price"),
        &CellValue::Number("12.345678901234567890".into())
    );
    assert_eq!(
        value(0, "big_number"),
        &CellValue::Number("18446744073709551615".into())
    );
    assert_eq!(
        value(0, "raw_data"),
        &CellValue::Binary("0x414200ff".into())
    );
    assert_eq!(profile.tls, TlsMode::VerifyIdentity);
    Ok(())
}
async fn rejection(engine: DbEngine, kind: &str) -> anyhow::Result<()> {
    let mut profile = profile(engine)?;
    match kind {
        "hostname" => {
            profile.host = "127.1".into();
            profile.port = port(engine, "WRONG_HOST_PORT")?;
            // Control: this exact forwarder reaches the TLS server with a valid
            // identity. Rejecting 127.1 is not merely a dead fixture endpoint.
            let mut control = profile.clone();
            control.host = "localhost".into();
            test_connection(&control, PASSWORD).await?;
        }
        "trust" => profile.ca_path = None,
        "https" => {
            profile.transport = Transport::HttpConnect {
                host: "localhost".into(),
                port: port(engine, "HTTPS_PORT")?,
                https: true,
            };
        }
        _ => unreachable!(),
    }
    let error = test_connection(&profile, PASSWORD)
        .await
        .expect_err("verification must reject this fixture");
    let message = format!("{error:#}");
    eprintln!("{} {kind} rejected: {message}", engine.display_name());
    let lower = message.to_lowercase();
    assert!(
        lower.contains("tls") || lower.contains("certificate") || lower.contains("ssl"),
        "expected TLS failure, not incidental connection failure: {message}"
    );
    Ok(())
}
macro_rules! tests {
    ($positive:ident, $http:ident, $host:ident, $trust:ident, $https:ident, $engine:expr) => {
        #[tokio::test]
        #[ignore = "requires disposable secure fixtures"]
        async fn $positive() -> anyhow::Result<()> {
            positive($engine, false).await
        }
        #[tokio::test]
        #[ignore = "requires disposable secure fixtures"]
        async fn $http() -> anyhow::Result<()> {
            positive($engine, true).await
        }
        #[tokio::test]
        #[ignore = "requires disposable secure fixtures"]
        async fn $host() -> anyhow::Result<()> {
            rejection($engine, "hostname").await
        }
        #[tokio::test]
        #[ignore = "requires disposable secure fixtures"]
        async fn $trust() -> anyhow::Result<()> {
            rejection($engine, "trust").await
        }
        #[tokio::test]
        #[ignore = "requires disposable secure fixtures"]
        async fn $https() -> anyhow::Result<()> {
            rejection($engine, "https").await
        }
    };
}
tests!(
    mysql_verified_tls,
    mysql_http_connect_verified_tls,
    mysql_wrong_hostname,
    mysql_untrusted_ca,
    mysql_untrusted_https,
    DbEngine::MySql
);
tests!(
    mariadb_verified_tls,
    mariadb_http_connect_verified_tls,
    mariadb_wrong_hostname,
    mariadb_untrusted_ca,
    mariadb_untrusted_https,
    DbEngine::MariaDb
);
