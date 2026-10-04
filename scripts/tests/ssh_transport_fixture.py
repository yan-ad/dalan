#!/usr/bin/env python3
"""Disposable real OpenSSH + MySQL/MariaDB fixtures (stdlib only).

Requires Docker, /usr/bin/ssh, ssh-keygen and cargo. Host trust comes directly
from a generated fixture public key, never ssh-keyscan/accept-new. Containers
share a private network; only SSH is exposed, on an ephemeral loopback port.
Database TLS is explicitly disabled: this tests SSH, not database TLS. No user
profiles, Keychain, SSH configuration or known_hosts are read or modified.
"""
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def key(directory, name):
    path = directory / name
    run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(path))
    return path


def main():
    run("docker", "info", stdout=subprocess.DEVNULL)
    suffix = uuid.uuid4().hex[:12]
    network = f"dalan-ssh-{suffix}"
    image = f"dalan-ssh-fixture:{suffix}"
    names = []
    try:
        # Spaces deliberately exercise the selected known_hosts option quoting.
        with tempfile.TemporaryDirectory(prefix="dalan ssh fixture ") as temp:
            directory = Path(temp).resolve()
            directory.chmod(0o755)
            host_key = key(directory, "host_key")
            client_key = key(directory, "client_key")
            bad_key = key(directory, "bad_key")
            bad_host_key = key(directory, "bad_host_key")
            (directory / "authorized_keys").write_text(client_key.with_suffix(".pub").read_text())
            root_password = secrets.token_hex(24)
            db_password = secrets.token_hex(24)
            # Reuse only the schema/typed rows, and give the fixture user SELECT
            # privileges; credentials are generated for this invocation only.
            sql = (ROOT / "scripts/tests/mysql-fixture.sql").read_text()
            sql = sql.replace("dalan-local-fixture-only", db_password)
            (directory / "fixture.sql").write_text(sql)
            (directory / "sshd_config").write_text(
                "Port 22\nListenAddress 0.0.0.0\nHostKey /fixture/host_key\n"
                "AuthorizedKeysFile /fixture/authorized_keys\nPermitRootLogin prohibit-password\n"
                "PubkeyAuthentication yes\nPasswordAuthentication no\n"
                "KbdInteractiveAuthentication no\nAllowTcpForwarding yes\n"
                "AllowAgentForwarding no\nX11Forwarding no\nPermitTTY no\n"
                "StrictModes yes\nLogLevel VERBOSE\n"
            )
            (directory / "Dockerfile").write_text(
                "FROM alpine:3.22\nRUN apk add --no-cache openssh openssl\n"
                "RUN mkdir -p /run/sshd && passwd -d root\n"
                'CMD ["/usr/sbin/sshd", "-D", "-e", "-f", "/fixture/sshd_config"]\n'
            )
            # Never send generated private keys/passwords to the image builder.
            (directory / ".dockerignore").write_text("*\n!Dockerfile\n")
            run("docker", "build", "--tag", image, str(directory))
            run("docker", "network", "create", "--label", "dalan.test-fixture=true", network,
                stdout=subprocess.DEVNULL)
            env = os.environ.copy()
            for engine, db_image, alias in [
                ("MYSQL", "mysql:8.4", "mysqlfixture"),
                ("MARIADB", "mariadb:11.4", "mariafixture"),
            ]:
                name = f"dalan-ssh-{engine.lower()}-{suffix}"
                names.append(name)
                run("docker", "run", "--detach", "--name", name,
                    "--label", "dalan.test-fixture=true", "--network", network,
                    "--network-alias", alias,
                    "--env", f"MYSQL_ROOT_PASSWORD={root_password}",
                    "--mount", f"type=bind,source={directory / 'fixture.sql'},target=/docker-entrypoint-initdb.d/fixture.sql,readonly",
                    db_image, "--require-secure-transport=OFF", stdout=subprocess.DEVNULL)
                deadline = time.monotonic() + 180
                while time.monotonic() < deadline:
                    ready = subprocess.run([
                        "docker", "exec", "--env", f"MYSQL_PWD={db_password}", name, "sh", "-c",
                        'if command -v mariadb >/dev/null; then client=mariadb; else client=mysql; fi; '
                        '"$client" -h127.0.0.1 -udalan_reader -N '
                        '-e "SELECT COUNT(*) FROM dalan_fixture.contact"'
                    ], text=True, capture_output=True)
                    if ready.returncode == 0 and ready.stdout.strip() == "3":
                        break
                    time.sleep(2)
                else:
                    run("docker", "logs", name)
                    raise RuntimeError(f"{engine} SSH fixture failed readiness")
                env[f"DALAN_SSH_{engine}_DBHOST"] = alias
            ssh_name = f"dalan-ssh-server-{suffix}"
            names.append(ssh_name)
            run("docker", "run", "--detach", "--name", ssh_name,
                "--label", "dalan.test-fixture=true", "--network", network,
                "--publish", "127.0.0.1::22",
                "--mount", f"type=bind,source={directory},target=/fixture,readonly",
                image, stdout=subprocess.DEVNULL)
            port = int(run("docker", "port", ssh_name, "22/tcp", capture_output=True)
                       .stdout.strip().rsplit(":", 1)[1])
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=1) as conn:
                        if conn.recv(256).startswith(b"SSH-2.0-"):
                            break
                except OSError:
                    pass
                time.sleep(0.2)
            else:
                run("docker", "logs", ssh_name)
                raise RuntimeError("SSH server did not become ready")
            for filename, public_key in [("known hosts", host_key), ("bad known hosts", bad_host_key)]:
                # Explicit host:port binding, from the fixture's owned public key.
                public = public_key.with_suffix(".pub").read_text().split()
                (directory / filename).write_text(f"[127.0.0.1]:{port} {public[0]} {public[1]}\n")
            env.update({
                "DALAN_SSH_HOST": "127.0.0.1", "DALAN_SSH_PORT": str(port),
                "DALAN_SSH_USER": "root", "DALAN_SSH_KEY": str(client_key),
                "DALAN_SSH_BAD_KEY": str(bad_key),
                "DALAN_SSH_KNOWN_HOSTS": str(directory / "known hosts"),
                "DALAN_SSH_BAD_KNOWN_HOSTS": str(directory / "bad known hosts"),
                "DALAN_SSH_DB_PASSWORD": db_password,
            })
            # An inherited agent must not make bad-key controls pass by accident.
            env.pop("SSH_AUTH_SOCK", None)
            print("Testing real SSH to both engines, selected host trust and identity rejection; database TLS disabled.", flush=True)
            # These are transport semantics tests, not a handshake load test.
            # Each case opens several independent tunnels against one disposable
            # sshd; serialize by default to avoid fixture startup contention.
            # Allow an explicit override for investigating parallel failures.
            test_args = sys.argv[1:]
            if not any(arg == "--test-threads" or arg.startswith("--test-threads=")
                       for arg in test_args):
                test_args = ["--test-threads=1", *test_args]
            result = subprocess.run([
                "cargo", "test", "-p", "dalan-drivers", "--test", "ssh_transport", "--locked",
                "--", "--ignored", "--nocapture", *test_args
            ], cwd=ROOT, env=env)
            return result.returncode
    finally:
        if names:
            subprocess.run(["docker", "rm", "-f", "-v", *names],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["docker", "network", "rm", network],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["docker", "image", "rm", image],
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
