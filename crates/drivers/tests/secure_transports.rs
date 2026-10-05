//! Opt-in disposable secure fixture tests; run `scripts/test-secure-transports`.
//! Never modifies the user's OS trust store or SSH known_hosts.
mod common;

use dalan_drivers::{
    BrowseRequest, CellValue, DbEngine, SourceProfile, TlsMode, Transport, browse, columns,
    discover_catalog, tables, test_connection,
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
    common::full_catalog(&profile, PASSWORD, "dalan_fixture").await?;
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
    common::queries(&profile, PASSWORD, &request).await?;
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
    let catalog_error = discover_catalog(&profile, PASSWORD)
        .await
        .expect_err("catalog must not bypass TLS verification");
    assert!(!format!("{catalog_error:#}").contains(PASSWORD));
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

async fn additional_modes(engine: DbEngine) -> anyhow::Result<()> {
    let mut p = profile(engine)?;
    p.host = "127.0.0.1".into();
    p.port = port(engine, "WRONG_HOST_PORT")?;
    p.tls = TlsMode::VerifyCa;
    // Trusted chain, deliberately mismatched identity is allowed only here.
    test_connection(&p, PASSWORD).await?;
    p.ca_path = None;
    assert!(test_connection(&p, PASSWORD).await.is_err());
    p.tls = TlsMode::Required;
    // Required encrypts but deliberately does not verify trust or identity.
    test_connection(&p, PASSWORD).await?;
    p.authentication = dalan_drivers::Authentication::NoAuth;
    p.username = "dalan_reader".into();
    // Passing a real account/password must not silently override NoAuth.
    let error = test_connection(&p, PASSWORD).await.unwrap_err();
    let message = error.to_string();
    assert!(
        !message.contains(PASSWORD),
        "NoAuth error leaked credentials"
    );
    // MySQL assigns unknown accounts a random decoy authentication plugin
    // (sql_authentication.cc: decoy_user), including sha256_password, which
    // mysql_async 0.37.1 does not implement. That fails closed before 1045.
    // MariaDB still requires an explicit access-denied response. Do not accept
    // transport, TLS, timeout, or arbitrary protocol failures as authentication.
    assert!(
        message == "Authentication denied; check password and account grants (code 1045)"
            || (engine == DbEngine::MySql
                && message == "Database requested an unsupported authentication plugin"),
        "NoAuth rejection had category {message}"
    );
    Ok(())
}
#[tokio::test]
#[ignore = "requires disposable secure fixture"]
async fn mysql_additional_tls_and_noauth_modes() -> anyhow::Result<()> {
    additional_modes(DbEngine::MySql).await
}
#[tokio::test]
#[ignore = "requires disposable secure fixture"]
async fn mariadb_additional_tls_and_noauth_modes() -> anyhow::Result<()> {
    additional_modes(DbEngine::MariaDb).await
}

/// Inspect the actual native handshake, not Debug options or a live server's
/// randomized decoy plugin. Neither a configured username nor the supplied
/// password may reach the wire, including after an authentication switch.
#[tokio::test]
async fn noauth_native_handshake_omits_credentials() -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn send(stream: &mut tokio::net::TcpStream, seq: u8, body: &[u8]) -> anyhow::Result<()> {
        let len = (body.len() as u32).to_le_bytes();
        stream.write_all(&[len[0], len[1], len[2], seq]).await?;
        stream.write_all(body).await?;
        Ok(())
    }
    async fn receive(stream: &mut tokio::net::TcpStream) -> anyhow::Result<Vec<u8>> {
        let mut header = [0; 4];
        stream.read_exact(&mut header).await?;
        let len = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
        anyhow::ensure!(len <= 4096, "mock handshake packet exceeds test cap");
        let mut body = vec![0; len];
        stream.read_exact(&mut body).await?;
        Ok(body)
    }

    for switch in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let p = SourceProfile {
            host: "127.0.0.1".into(),
            port: listener.local_addr()?.port(),
            username: "must-not-be-sent".into(),
            authentication: dalan_drivers::Authentication::NoAuth,
            tls: TlsMode::Disabled,
            ..SourceProfile::default()
        };
        let server = async {
            let (mut stream, _) = listener.accept().await?;
            // Protocol 10, CLIENT_PROTOCOL_41 | SECURE_CONNECTION | PLUGIN_AUTH.
            let capabilities: u32 = 0x0008_8200;
            let mut handshake = b"\x0a8.4.11\0\x01\0\0\0abcdefgh\0".to_vec();
            handshake.extend_from_slice(&(capabilities as u16).to_le_bytes());
            handshake.extend_from_slice(&[45, 2, 0]);
            handshake.extend_from_slice(&((capabilities >> 16) as u16).to_le_bytes());
            handshake.push(21);
            handshake.extend_from_slice(&[0; 10]);
            handshake.extend_from_slice(b"ijklmnopqrst\0caching_sha2_password\0");
            send(&mut stream, 0, &handshake).await?;
            let response = receive(&mut stream).await?;
            anyhow::ensure!(response.len() >= 34, "native handshake is truncated");
            // Fixed protocol-41 header is 32 bytes; username NUL and empty
            // length-encoded/secure authentication response follow it.
            assert!(response[32] == 0, "NoAuth sent a username");
            assert!(response[33] == 0, "NoAuth sent an authentication proof");
            assert!(
                !response
                    .windows(PASSWORD.len())
                    .any(|w| w == PASSWORD.as_bytes()),
                "NoAuth sent a password"
            );
            if switch {
                send(
                    &mut stream,
                    2,
                    b"\xfesha256_password\0abcdefghijklmnopqrst\0",
                )
                .await?;
                // The unsupported-plugin branch may drop its buffered empty
                // packet before flushing. Inspect every byte received through
                // EOF: either no packet or an empty packet, never a proof.
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).await?;
                assert!(
                    bytes.is_empty() || bytes == [0, 0, 0, 3],
                    "NoAuth sent a switched authentication proof"
                );
            } else {
                send(&mut stream, 2, b"\xff\x15\x04#28000denied").await?;
            }
            Ok::<_, anyhow::Error>(())
        };
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let (server_result, result) = tokio::join!(server, test_connection(&p, PASSWORD));
            let message = result.expect_err("mock must reject NoAuth").to_string();
            assert!(
                !message.contains(PASSWORD),
                "NoAuth error leaked credentials"
            );
            server_result
                .map_err(|_| anyhow::anyhow!("mock exchange failed; client category {message}"))?;
            assert!(
                !message.contains(PASSWORD),
                "NoAuth error leaked credentials"
            );
            let expected = if switch {
                "Database requested an unsupported authentication plugin"
            } else {
                "Authentication denied; check password and account grants (code 1045)"
            };
            assert!(
                message == expected,
                "NoAuth rejection had category {message}"
            );
            Ok::<_, anyhow::Error>(())
        })
        .await??;
    }
    Ok(())
}
