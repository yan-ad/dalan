//! Real OpenSSH integration; opt in with `scripts/test-ssh-transport`.
//! Disposable trust/key files only. Database TLS is explicitly disabled here;
//! this suite validates SSH encryption/authentication, not database TLS.
mod common;

use dalan_drivers::{
    BrowseRequest, CellValue, DbEngine, SourceProfile, TlsMode, Transport, browse, columns,
    test_connection,
};

fn profile(engine: DbEngine) -> anyhow::Result<SourceProfile> {
    let name = match engine {
        DbEngine::MySql => "MYSQL",
        DbEngine::MariaDb => "MARIADB",
    };
    Ok(SourceProfile {
        engine,
        host: std::env::var(format!("DALAN_SSH_{name}_DBHOST"))?,
        port: 3306,
        username: "dalan_reader".into(),
        tls: TlsMode::Disabled,
        transport: Transport::Ssh {
            host: std::env::var("DALAN_SSH_HOST")?,
            port: std::env::var("DALAN_SSH_PORT")?.parse()?,
            user: std::env::var("DALAN_SSH_USER")?,
            identity_file: Some(std::env::var("DALAN_SSH_KEY")?),
            known_hosts_file: Some(std::env::var("DALAN_SSH_KNOWN_HOSTS")?),
        },
        ..SourceProfile::default()
    })
}

async fn positive(engine: DbEngine) -> anyhow::Result<()> {
    let profile = profile(engine)?;
    let password = std::env::var("DALAN_SSH_DB_PASSWORD")?;
    let report = test_connection(&profile, &password).await?;
    assert!(!report.server_version.is_empty());
    assert!(report.databases.iter().any(|db| db == "dalan_fixture"));
    common::full_catalog(&profile, &password, "dalan_fixture").await?;
    let metadata = columns(&profile, &password, "dalan_fixture", "contact").await?;
    let request = BrowseRequest {
        database: "dalan_fixture".into(),
        table: "contact".into(),
        limit: 2,
        ..BrowseRequest::default()
    };
    common::sorted_pages(&profile, &password, &request).await?;
    common::queries(&profile, &password, &request).await?;
    let page = browse(&profile, &password, &request).await?;
    assert_eq!(page.rows.len(), 2);
    assert!(page.has_more);
    let value = |row: usize, name: &str| {
        &page.rows[row][metadata.iter().position(|c| c.name == name).unwrap()]
    };
    assert_eq!(value(0, "name"), &CellValue::Text("Alice".into()));
    assert_eq!(value(1, "email"), &CellValue::Null);
    assert_eq!(
        value(0, "big_number"),
        &CellValue::Number("18446744073709551615".into())
    );
    assert_eq!(
        value(0, "raw_data"),
        &CellValue::Binary("0x414200ff".into())
    );
    eprintln!(
        "{} real SSH transport: {}",
        engine.display_name(),
        report.server_version
    );
    Ok(())
}

async fn rejection(engine: DbEngine, wrong_key: bool) -> anyhow::Result<()> {
    let mut profile = profile(engine)?;
    let password = std::env::var("DALAN_SSH_DB_PASSWORD")?;
    // Prove the exact endpoint/credentials work before changing only trust/key.
    test_connection(&profile, &password).await?;
    if let Transport::Ssh {
        identity_file,
        known_hosts_file,
        ..
    } = &mut profile.transport
    {
        if wrong_key {
            *identity_file = Some(std::env::var("DALAN_SSH_BAD_KEY")?);
        } else {
            *known_hosts_file = Some(std::env::var("DALAN_SSH_BAD_KNOWN_HOSTS")?);
        }
    }
    profile.validate()?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(25),
        test_connection(&profile, &password),
    )
    .await?;
    let error = result.expect_err("SSH must not fall back after selected trust/key rejection");
    eprintln!(
        "{} SSH {} rejected: {error:#}",
        engine.display_name(),
        if wrong_key { "key" } else { "trust" }
    );
    Ok(())
}
macro_rules! tests {
    ($success:ident, $trust:ident, $key:ident, $engine:expr) => {
        #[tokio::test]
        #[ignore = "requires disposable SSH fixtures"]
        async fn $success() -> anyhow::Result<()> {
            positive($engine).await
        }
        #[tokio::test]
        #[ignore = "requires disposable SSH fixtures"]
        async fn $trust() -> anyhow::Result<()> {
            rejection($engine, false).await
        }
        #[tokio::test]
        #[ignore = "requires disposable SSH fixtures"]
        async fn $key() -> anyhow::Result<()> {
            rejection($engine, true).await
        }
    };
}
tests!(
    mysql_ssh,
    mysql_wrong_known_host,
    mysql_wrong_selected_key,
    DbEngine::MySql
);
tests!(
    mariadb_ssh,
    mariadb_wrong_known_host,
    mariadb_wrong_selected_key,
    DbEngine::MariaDb
);
