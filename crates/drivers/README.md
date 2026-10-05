# Connection profile contract

Existing serialized profiles continue to deserialize: the new endpoint defaults to
`Default`, authentication to `UserPassword`, schema selection to `All`, and source
options to 10-second connect / 20-second query timeouts / 100-row page size. Legacy
SSH transports default `parse_config` to false.

- `Default` uses the host, port, and optional default database fields.
- `UrlOnly` uses a credential-free `mysql://` or `mariadb://` URL (optionally prefixed
  `jdbc:`). Both engines accept either scheme because the protocol is compatible.
  Host, port, and database fields are ignored, including a stale default database
  when the URL has no database. IPv6 literals and UTF-8 percent-encoded database
  names are supported. One path segment is accepted; an encoded slash belongs to
  the database name. Userinfo, query options, and fragments are rejected with
  static, credential-safe errors. Configure authentication, TLS, and timeouts in
  their dedicated fields, not JDBC URL options.
- `UnixSocket` requires an absolute local socket path, direct transport, a Unix
  platform, and explicitly disabled database TLS. The native connector cannot
  encrypt Unix streams; verified/encrypted TLS modes are rejected instead of
  silently downgraded. Socket validation does not open the socket.

`connection_target()` returns effective routing fields. `resolved()` and its alias
`effective_profile()` validate and return a cloned normalized profile, replacing
URL mode with `Default` while retaining explicit socket mode. Driver operations
resolve profiles themselves. `canonical_url()` emits a credential-free MySQL URL
for TCP mode, preserves a validated URL-mode input, and errors for socket mode.

`NoAuth` sends no native username/password, regardless of the supplied username or
password argument; it does not fall back to root or cached credentials. The server
may reject an anonymous connection. Passwords are never persisted in profiles.

Schema selection is **display filtering, not database authorization**. `Selected`
with an empty vector shows no schemas. `visible_schema()` provides the pure filter.
Discovery ignores this setting so callers can cache complete metadata; the optional
default database retains the existing discovery-scoping behavior. Least-privilege
server grants remain required.

TLS `VerifyIdentity` validates trust and hostname. `VerifyCa` validates trust but
**does not authenticate the hostname**. `Required` encrypts traffic but **does not
verify trust or hostname** and is vulnerable to interception. `Disabled` requests
no database TLS. None of these modes automatically retries insecurely. Optional
client certificate/key paths must both be present and absolute, without control
characters. Validation does not read these files; PEM/DER identity loading occurs
only during connection. Encrypted private keys/passphrases are not supported by
this profile API. Client-identity wiring is covered by compilation/validation, not
by a live mutual-TLS success claim.

Connect timeouts are strictly 1–60 seconds; query timeouts are 1–120 seconds; page
sizes are 1–200 rows. Connection setup has its own timeout, and ordinary operations
have an outer configured query timeout (so a shorter query deadline can cancel
connection setup). Discovery keeps a 120-second global cap and applies configured
query deadlines to individual metadata reads. Read-only console queries also set
engine-specific server execution deadlines. Callers use `options.page_size` when
constructing browse/query requests; explicit request limits remain authoritative.

SSH `parse_config: true` explicitly opts into local OpenSSH configuration, including
configured ProxyCommand execution. The default supplies `-F /dev/null`. Command-line
strict known-host verification, batch mode, disabled password/interactive auth,
non-forwarded agents, and selected identity/trust files remain authoritative. The
optional `ssh_configuration_id` is a UUID metadata reference only: it never resolves
or activates SSH by itself; callers must materialize a transport before connecting.

Disposable verification: `cargo test -p dalan-drivers --lib`,
`cargo clippy -p dalan-drivers --all-targets -- -D warnings`,
`scripts/test-databases`, `scripts/test-secure-transports`, and
`scripts/test-ssh-transport`. Unix live tests use actual Unix listeners forwarding
to disposable servers; they do not require host/container socket mounts.
