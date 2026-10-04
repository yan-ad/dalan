//! Owned, single-connection relays; dropping the connection owner aborts forwarding.
use crate::sources::{SourceProfile, Transport};
use anyhow::{Result, anyhow, ensure};
use std::{
    net::{IpAddr, Ipv4Addr},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
    task::JoinHandle,
};

pub(crate) struct Relay {
    pub port: u16,
    task: JoinHandle<Result<()>>,
    failure: Arc<Mutex<Option<String>>>,
}
impl Drop for Relay {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Relay {
    pub fn ips(&self) -> Vec<IpAddr> {
        vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]
    }
    pub fn check_failure(&self) -> Result<()> {
        if let Some(message) = self
            .failure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            return Err(anyhow!("{message}"));
        }
        Ok(())
    }
    pub fn check(&self) -> Result<()> {
        self.check_failure()?;
        ensure!(
            !self.task.is_finished(),
            "Transport relay closed unexpectedly (check proxy or SSH configuration)"
        );
        Ok(())
    }
}
// Store only locally generated labels and the typed I/O kind, never OS text,
// SSH stderr, proxy responses, hostnames, or forwarded protocol bytes.
fn spawn_forward(
    failure: Arc<Mutex<Option<String>>>,
    label: &'static str,
    future: impl std::future::Future<Output = Result<()>> + Send + 'static,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let result = future.await;
        if let Err(error) = &result {
            let message = match error.downcast_ref::<std::io::Error>() {
                Some(e) => format!("{label} failed ({:?})", e.kind()),
                None => format!("{label} closed unexpectedly; check transport configuration"),
            };
            *failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
        }
        result
    })
}
fn authority(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}
/// Byte-at-a-time parsing deliberately never consumes tunneled protocol bytes.
async fn connect_response<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    target: &str,
) -> Result<()> {
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .await?;
    stream.flush().await?;
    let mut header = Vec::new();
    loop {
        ensure!(
            header.len() < 16 * 1024,
            "HTTP CONNECT response headers exceed 16 KiB"
        );
        header.push(stream.read_u8().await?);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let line = header.split(|b| *b == b'\n').next().unwrap_or_default();
    let line = std::str::from_utf8(line).map_err(|_| anyhow!("Invalid HTTP CONNECT response"))?;
    let mut parts = line.split_whitespace();
    ensure!(
        matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")),
        "Invalid HTTP CONNECT response protocol"
    );
    let code = parts
        .next()
        .and_then(|v| v.parse::<u16>().ok())
        .ok_or_else(|| anyhow!("Invalid HTTP CONNECT status"))?;
    ensure!(
        (200..300).contains(&code),
        "HTTP CONNECT proxy rejected the tunnel (status {code})"
    );
    Ok(())
}
pub(crate) async fn start(profile: &SourceProfile) -> Result<Option<Relay>> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let target = authority(&profile.host, profile.port);
    let failure = Arc::new(Mutex::new(None));
    let task = match &profile.transport {
        // Own the TCP transport even without a proxy, so caps/cancellation can
        // close it without mysql_async's drop cleanup draining unread rows.
        Transport::Direct => {
            let mut remote = TcpStream::connect((profile.host.as_str(), profile.port))
                .await
                .map_err(|e| anyhow!("Cannot connect to database ({:?})", e.kind()))?;
            spawn_forward(failure.clone(), "Database TCP relay", async move {
                let (mut local, _) = listener.accept().await?;
                tokio::io::copy_bidirectional(&mut local, &mut remote).await?;
                Ok(())
            })
        }
        Transport::Ssh {
            host,
            port,
            user,
            identity_file,
            known_hosts_file,
        } => {
            let mut cmd = Command::new("/usr/bin/ssh");
            cmd.kill_on_drop(true)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            cmd.args([
                "-F",
                "/dev/null",
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ConnectTimeout=10",
                "-o",
                "PasswordAuthentication=no",
                "-o",
                "KbdInteractiveAuthentication=no",
                "-o",
                "ForwardAgent=no",
                "-o",
                "ClearAllForwardings=yes",
                "-p",
                &port.to_string(),
                "-l",
                user,
            ]);
            if let Some(path) = identity_file {
                cmd.args(["-o", "IdentitiesOnly=yes", "-i"]).arg(path);
            }
            if let Some(path) = known_hosts_file {
                // OpenSSH parses -o values as configuration, even though no shell
                // is involved. Quote spaces and escape quotes/backslashes so the
                // selected absolute filename stays a single literal argument.
                let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
                cmd.arg("-o")
                    .arg(format!("UserKnownHostsFile=\"{escaped}\""));
                // A selected trust file is authoritative, not supplemented by
                // machine-wide trust. None preserves OpenSSH's usual defaults.
                cmd.args(["-o", "GlobalKnownHostsFile=/dev/null"]);
            }
            cmd.arg("-W").arg(target).arg("--").arg(host);
            let mut child = cmd
                .spawn()
                .map_err(|_| anyhow!("Unable to start /usr/bin/ssh"))?;
            let mut input = child
                .stdin
                .take()
                .ok_or_else(|| anyhow!("SSH stdin unavailable"))?;
            let mut output = child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("SSH stdout unavailable"))?;
            spawn_forward(failure.clone(), "SSH tunnel", async move {
                let (socket, _) = listener.accept().await?;
                let (mut read, mut write) = socket.into_split();
                let forward = async {
                    tokio::try_join!(
                        tokio::io::copy(&mut read, &mut input),
                        tokio::io::copy(&mut output, &mut write)
                    )?;
                    Result::<()>::Ok(())
                };
                tokio::select! { r = forward => r?, _ = child.wait() => return Err(anyhow!("SSH tunnel exited (check key, agent, and known hosts)")) }
                Ok(())
            })
        }
        Transport::HttpConnect { host, port, https } => {
            let host = host.clone();
            let port = *port;
            let https = *https;
            // Establish and verify the proxy before returning a local listener.
            let socket = TcpStream::connect((host.as_str(), port))
                .await
                .map_err(|_| anyhow!("Cannot connect to HTTP proxy"))?;
            if https {
                let connector = native_tls::TlsConnector::new()
                    .map_err(|_| anyhow!("Cannot initialize proxy TLS"))?;
                let mut stream = tokio_native_tls::TlsConnector::from(connector)
                    .connect(&host, socket)
                    .await
                    .map_err(|_| {
                        anyhow!("Proxy TLS certificate or connection verification failed")
                    })?;
                tokio::time::timeout(
                    Duration::from_secs(10),
                    connect_response(&mut stream, &target),
                )
                .await
                .map_err(|_| anyhow!("HTTP CONNECT timed out"))??;
                spawn_forward(failure.clone(), "HTTPS CONNECT relay", async move {
                    let (mut local, _) = listener.accept().await?;
                    tokio::io::copy_bidirectional(&mut local, &mut stream).await?;
                    Ok(())
                })
            } else {
                let mut stream = socket;
                tokio::time::timeout(
                    Duration::from_secs(10),
                    connect_response(&mut stream, &target),
                )
                .await
                .map_err(|_| anyhow!("HTTP CONNECT timed out"))??;
                spawn_forward(failure.clone(), "HTTP CONNECT relay", async move {
                    let (mut local, _) = listener.accept().await?;
                    tokio::io::copy_bidirectional(&mut local, &mut stream).await?;
                    Ok(())
                })
            }
        }
    };
    Ok(Some(Relay {
        port,
        task,
        failure,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn forwarding_failure_retains_only_safe_io_kind() {
        let failure = Arc::new(Mutex::new(None));
        let task = spawn_forward(failure.clone(), "Database TCP relay", async {
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "sentinel-secret-host-password",
            )
            .into())
        });
        let relay = Relay {
            port: 1,
            task,
            failure,
        };
        while !relay.task.is_finished() {
            tokio::task::yield_now().await;
        }
        let error = relay.check_failure().unwrap_err();
        assert_eq!(
            error.to_string(),
            "Database TCP relay failed (ConnectionReset)"
        );
        assert!(!format!("{error:#?}").contains("sentinel"));
        assert_eq!(error.chain().count(), 1);
        assert!(relay.check().is_err());
    }
    async fn response(bytes: Vec<u8>) -> Result<Vec<u8>> {
        let (mut a, mut b) = tokio::io::duplex(32768);
        let writer = tokio::spawn(async move {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(b.read_u8().await.unwrap());
            }
            b.write_all(&bytes).await.unwrap();
        });
        let result = connect_response(&mut a, "db:3306").await;
        if result.is_err() {
            writer.abort();
            return result.map(|_| Vec::new());
        }
        let mut rest = Vec::new();
        a.read_to_end(&mut rest).await?;
        writer.await?;
        Ok(rest)
    }
    #[tokio::test]
    async fn connect_success_preserves_protocol_bytes() {
        assert_eq!(
            response(b"HTTP/1.1 200 OK\r\n\r\nMYSQL".to_vec())
                .await
                .unwrap(),
            b"MYSQL"
        );
    }
    #[tokio::test]
    async fn connect_rejects_non_2xx_and_socks() {
        assert!(response(b"HTTP/1.1 407 No\r\n\r\n".to_vec()).await.is_err());
        assert!(response(b"SOCKS 200 OK\r\n\r\n".to_vec()).await.is_err());
    }
    #[tokio::test]
    async fn connect_caps_headers() {
        let mut bytes = b"HTTP/1.1 200 OK\r\nX: ".to_vec();
        bytes.extend(vec![b'a'; 16384]);
        bytes.extend(b"\r\n\r\n");
        assert!(response(bytes).await.is_err());
    }
}
