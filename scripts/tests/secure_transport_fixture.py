#!/usr/bin/env python3
"""Stdlib-only disposable verified database TLS fixtures.

Requires Docker, openssl and cargo. Certificates, anonymous CONNECT proxies and
containers are ephemeral. Does not modify OS trust or the user's SSH files.
HTTPS is deliberately untrusted by the proxy connector: its rejection is tested,
not misrepresented as a positive HTTPS route. SSH requires an already trusted
host and is not provisioned by this script.
"""
import contextlib
import os
from pathlib import Path
import select
import signal
import socket
import socketserver
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def certificate(directory, name, cn, extensions):
    run("openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj",
        f"/CN={cn}", "-keyout", str(directory / f"{name}.key"), "-out",
        str(directory / f"{name}.csr"), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    ext = directory / f"{name}.ext"
    ext.write_text(extensions)
    run("openssl", "x509", "-req", "-in", str(directory / f"{name}.csr"),
        "-CA", str(directory / "ca.pem"), "-CAkey", str(directory / "ca.key"),
        "-CAcreateserial", "-days", "2", "-extfile", str(ext), "-out",
        str(directory / f"{name}.crt"), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    # Only these disposable server keys must be readable by the container UID.
    (directory / f"{name}.key").chmod(0o644)


class Forwarder(socketserver.ThreadingTCPServer):
    daemon_threads = True
    allow_reuse_address = True


class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            client = self.request
            if self.server.tls:
                client = self.server.tls.wrap_socket(client, server_side=True)
            with client:
                if self.server.connect:
                    data = b""
                    while b"\r\n\r\n" not in data and len(data) < 8192:
                        chunk = client.recv(1)
                        if not chunk:
                            return
                        data += chunk
                    expected = f"CONNECT localhost:{self.server.target[1]} HTTP/1.1".encode()
                    if data.split(b"\r\n", 1)[0] != expected:
                        client.sendall(b"HTTP/1.1 403 Forbidden\r\n\r\n")
                        return
                    client.sendall(b"HTTP/1.1 200 Connection established\r\n\r\n")
                with socket.create_connection(self.server.target, timeout=10) as upstream:
                    client.settimeout(15)
                    upstream.settimeout(15)
                    while True:
                        ready, _, _ = select.select([client, upstream], [], [], 15)
                        if not ready:
                            return
                        for source in ready:
                            data = source.recv(65536)
                            if not data:
                                return
                            (upstream if source is client else client).sendall(data)
        except (OSError, ssl.SSLError):
            # Failed TLS verification and connection teardown are expected tests.
            pass


@contextlib.contextmanager
def forward(target, host="127.0.0.1", connect=False, tls=None):
    server = Forwarder((host, 0), Handler)
    server.target, server.connect, server.tls = target, connect, tls
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_address[1]
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


def main():
    run("docker", "info", stdout=subprocess.DEVNULL)
    names = []
    try:
        with tempfile.TemporaryDirectory(prefix="dalan-secure-") as temp, contextlib.ExitStack() as stack:
            certs = Path(temp)
            certs.chmod(0o755)
            run("openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2",
                "-subj", "/CN=Dalan disposable fixture CA", "-keyout", str(certs / "ca.key"),
                "-out", str(certs / "ca.pem"), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            certificate(certs, "server", "localhost",
                        "subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\n")
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(certs / "server.crt", certs / "server.key")
            env = os.environ.copy()
            env["DALAN_SECURE_CA_PATH"] = str(certs / "ca.pem")
            for engine, image in [("MYSQL", "mysql:8.4"), ("MARIADB", "mariadb:11.4")]:
                name = f"dalan-secure-{engine.lower()}-{uuid.uuid4().hex[:10]}"
                names.append(name)
                run("docker", "run", "--detach", "--name", name,
                    "--label", "dalan.test-fixture=true", "--publish", "127.0.0.1::3306",
                    "--env", "MYSQL_ROOT_PASSWORD=dalan-local-root-only",
                    "--mount", f"type=bind,source={certs},target=/certs,readonly",
                    "--mount", f"type=bind,source={ROOT / 'scripts/tests/mysql-fixture.sql'},target=/docker-entrypoint-initdb.d/fixture.sql,readonly",
                    image, "--ssl-ca=/certs/ca.pem", "--ssl-cert=/certs/server.crt",
                    "--ssl-key=/certs/server.key", "--require-secure-transport=ON",
                    stdout=subprocess.DEVNULL)
                deadline = time.monotonic() + 180
                while time.monotonic() < deadline:
                    ready = subprocess.run([
                        "docker", "exec", name, "sh", "-c",
                        'if command -v mariadb >/dev/null; then client=mariadb; else client=mysql; fi; '
                        'MYSQL_PWD=dalan-local-fixture-only "$client" -h127.0.0.1 -udalan_reader '
                        '-N -e "SELECT COUNT(*) FROM dalan_fixture.contact"'
                    ], text=True, capture_output=True)
                    if ready.returncode == 0 and ready.stdout.strip() == "3":
                        break
                    time.sleep(2)
                else:
                    run("docker", "logs", name)
                    raise RuntimeError(f"{engine} TLS fixture failed readiness")
                port = int(run("docker", "port", name, "3306/tcp", capture_output=True).stdout.strip().rsplit(":", 1)[1])
                env[f"DALAN_SECURE_{engine}_PORT"] = str(port)
                target = ("127.0.0.1", port)
                # Legacy numeric hostname 127.1 resolves to loopback, but is absent from SAN.
                for suffix, host, connect, tls in [
                    ("WRONG_HOST_PORT", "127.0.0.1", False, None),
                    ("HTTP_PORT", "127.0.0.1", True, None),
                    ("HTTPS_PORT", "127.0.0.1", True, context),
                ]:
                    proxy_port = stack.enter_context(forward(target, host, connect, tls))
                    env[f"DALAN_SECURE_{engine}_{suffix}"] = str(proxy_port)
            print("Testing verified database TLS, hostname/trust rejection, HTTP CONNECT + verified database TLS, and untrusted HTTPS rejection.", flush=True)
            result = subprocess.run(["cargo", "test", "-p", "dalan-drivers", "--test",
                                     "secure_transports", "--locked", "--", "--ignored",
                                     "--nocapture", *sys.argv[1:]], cwd=ROOT, env=env)
            return result.returncode
    finally:
        if names:
            subprocess.run(["docker", "rm", "-f", "-v", *names],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == "__main__":
    def interrupted(signum, _frame):
        raise KeyboardInterrupt(f"signal {signum}")
    for sig in (signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, interrupted)
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
