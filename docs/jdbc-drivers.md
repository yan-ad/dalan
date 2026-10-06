# Optional JDBC drivers

Status: first experimental JDBC bridge, curated catalog and explicit installer are implemented. This is not vendor/server qualification, a sandbox, or a general dependency/package manager. Built-in native adapters remain available without Java; see the [native matrix](native-drivers.md).

## Install and connect

1. Open **Data Sources and Drivers → Drivers**. The searchable left list distinguishes built-in and JDBC entries, with **All / Installed / Available** filters. Select a JDBC entry to see available versions on the right.
2. Use **Refresh** explicitly to fetch public catalog/version metadata. Opening the dialog does not automatically contact remote catalogs. Catalog results are cached in memory only, not as an offline version feed.
3. Select a version, choose **Install**, and confirm the third-party-code trust warning. Installation can fail closed even for a listed version; availability is not an installability or compatibility guarantee.
4. Supply an explicit absolute path to a local **Java 17+ executable** and use **Check Java**. Dalan neither discovers/installs a managed JRE nor bundles a JVM.
5. Use **Create connection**, or select an installed driver in the source form. The installed driver class and JAR paths are read-only derived fields, not arbitrary class/JAR import controls. Enter a credential-free vendor JDBC URL, and put credentials in the explicit authentication fields. Source Apply/OK persists the profile through the existing source-management flow.

JDBC profiles use a distinct JDBC engine and installed-driver identity; native engine preferences still mean Latest bundled/exact bundled pin, not a Maven version. Imports remain MySQL/MariaDB-only. There is no arbitrary JAR import or uninstall UI yet.

## Catalog and installation boundaries

The curated seven entries are **H2, SQLite, DuckDB, SQL Server, Oracle, MariaDB and MySQL**. Discovery checks `https://frameworks.jetbrains.com/jdbc-drivers/jdbc-drivers.json`; that metadata is discovery only, never authority for downloaded ZIPs, executable classes or arbitrary artifact URLs. No JetBrains ZIP is downloaded. Versions come from `https://search.maven.org/solrsearch/select`, using curated group/artifact coordinates, `core=gav` and at most 50 results per entry. Each metadata response is bounded to **2 MiB**.

JetBrains discovery failure falls back to curated coordinates and Maven versions with an unavailable-metadata note. A Maven failure for any entry fails the entire refresh; there is no partial successful version refresh or invented fallback version list.

Installation downloads only approved coordinates from `https://repo.maven.apache.org/maven2` over HTTPS with redirects disabled. Each JAR requires its repository **SHA-256 sidecar** and matching bytes. A missing/404 sidecar fails installation closed. JARs are capped at **64 MiB each / 128 MiB total**, checked as archives without extraction, and saved in private staging with an atomic manifest/install publication that does not overwrite an existing version. Installed manifests, paths and hashes are validated on load; hidden incomplete staging is not offered as an installed driver.

SQLite's curated auxiliary artifacts are `slf4j-api` and `slf4j-nop` **1.7.36**, also requiring sidecars. Public sidecar probes have encountered 404 responses, including these auxiliary artifacts. Oracle/MariaDB/MySQL version probes also encountered 404 responses; a version listing is not proof of successful installation. Do not bypass checksums to make a catalog entry installable. This is a small trusted coordinate/class mapping, **not** a general transitive dependency resolver. Repository checksums verify matching bytes, not publisher signatures, provenance or safety; review vendor licensing independently.

## Runtime and credential boundary

Each operation launches a fresh child using the Java 17+ **source-file launcher** and `-Xmx256m`. The child has a cleared environment. A bounded **1 MiB stdin JSON** request carries the URL, explicit credentials and operation; passwords are never command-line arguments or environment variables. The helper redirects driver stdout away from the protocol, disables DriverManager logging, and returns static categorized errors rather than exception/credential logs; the Rust launcher discards child stderr. JSON responses are capped at **2 MiB** and checked before use.

Every operation verifies configured JAR SHA-256 hashes and executes a copied, verified temporary snapshot, rather than later reopening a mutable installed pathname. JAR manifest **Class-Path** entries are rejected to prevent silently extending the configured classpath.

**Process isolation is not a sandbox.** A JAR is arbitrary third-party code running with the user's OS privileges. It can access files/network, spawn processes or misbehave. Timeout/cancellation kills the owned child, not driver-created descendants. The heap argument and retained response/preview caps are not bounds on all JVM/native memory, driver wire allocations, remote server work or descendant resource usage.

## Source URL and TLS policy

Vendor URLs must be credential-free. Validation rejects embedded user/password forms (including Oracle thin syntax), percent-decoded credential/property bypasses and dangerous initialization properties such as `INIT`. Error messages do not echo rejected credentials. These restrictions are not a complete vendor-property compatibility guarantee.

JDBC sources are direct-only. Native transport and TLS controls are disabled/rejected for JDBC: no inherited SSH/Unix/CONNECT or native certificate policy. **The vendor URL owns JDBC TLS configuration.** Dalan does not translate native VerifyIdentity/VerifyCa settings into vendor properties or configure a Java truststore; review the driver's documented TLS behavior yourself.

## Restricted read operations

Connections must accept `setReadOnly(true)` and verify `isReadOnly()`, with autocommit disabled and rollback on completion. Generic SQL consoles accept a conservative AST-validated single SELECT and a safe aggregate/function subset; executable comments and writes are blocked. This is not arbitrary SQL, transaction console or vendor-function parity, and driver read-only flags are not an OS/server sandbox.

Metadata uses JDBC catalog/schema identities and quoted identifiers for object browsing. Catalog-less drivers expose a reserved `(connection default)` scope instead of an empty tree label; that scope keeps the vendor URL’s default. Source/cache identity includes the JDBC URL, runtime, class and artifact hashes, so switching versions/targets invalidates old cached metadata and tabs. Browse offsets are bounded to **10,000**; native WHERE/ORDER BY clause fragments and structured filtering/sorting are unsupported for this first bridge. Table writes/staged Apply are disabled for JDBC, even where a vendor would support them.

## Evidence and remaining gates

An actual temporary JDK compiled the owned fake driver/helper and successfully exercised Java source launching, JDBC class loading and secret/log redaction. The opt-in helper is:

```sh
./scripts/test-jdbc-bridge --jdk /absolute/path/to/jdk
```

It uses temporary synthetic files and does not download a JDK/JAR or contact a database. Public metadata and sidecar probes are not a full artifact installation test. **No real vendor/server qualification or complete downloaded-JAR install/execution test has run.** Local validation passed 224 headless units + one wire integration, 235 production UI and 14 Python tests, strict lint/formatting and signed debug bundle checks; see [testing](testing.md#first-jdbc-bridge-and-installer). Native UI interaction, OS permission/process-tree behavior, driver authentication/TLS matrices, signatures/publisher trust, managed Java, arbitrary JAR import, uninstall and broader worker contracts remain future gates. No hosted CI or private configuration/database access is claimed by this docs update.
