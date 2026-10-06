//! PostgreSQL User & Privileges administration logic.
//!
//! Adapted and ported from DBX's PostgreSQL role administration patterns
//! (`target/research/dbx/apps/desktop/src/lib/database/databaseUserAdmin.ts`).
//! Provides catalog queries, DCL generators, safe identifier/literal quoting,
//! and parsing of PostgreSQL role metadata and grants.

use anyhow::{Result, ensure};

pub const POSTGRES_DATABASE_PRIVILEGES: &[&str] = &["CONNECT", "CREATE", "TEMPORARY"];
pub const POSTGRES_SCHEMA_PRIVILEGES: &[&str] = &["USAGE", "CREATE"];
pub const POSTGRES_TABLE_PRIVILEGES: &[&str] = &[
    "SELECT",
    "INSERT",
    "UPDATE",
    "DELETE",
    "TRUNCATE",
    "REFERENCES",
    "TRIGGER",
];

/// PostgreSQL Role representation containing identity and privilege flags.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PostgresRole {
    pub name: String,
    pub can_login: bool,
    pub is_superuser: bool,
    pub can_create_db: bool,
    pub can_create_role: bool,
    pub replication: bool,
    pub bypass_rls: bool,
}

impl PostgresRole {
    pub fn kind_label(&self) -> &'static str {
        if self.can_login { "LOGIN" } else { "ROLE" }
    }

    /// Formats an attributes badge label matching DBX (e.g., "LOGIN", "LOGIN • SUPERUSER").
    pub fn badge_label(&self) -> String {
        let mut badges = Vec::new();
        if self.can_login {
            badges.push("LOGIN");
        }
        if self.is_superuser {
            badges.push("SUPERUSER");
        }
        if self.can_create_db {
            badges.push("CREATEDB");
        }
        if self.can_create_role {
            badges.push("CREATEROLE");
        }
        if self.replication {
            badges.push("REPLICATION");
        }
        if self.bypass_rls {
            badges.push("BYPASSRLS");
        }
        if badges.is_empty() {
            "ROLE".to_string()
        } else {
            badges.join(" • ")
        }
    }
}

/// Category classification for formatted grant output lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PostgresGrantCategory {
    Role,
    Attributes,
    MemberOf,
    HasMember,
    Database,
    Schema,
    Table,
    Other,
}

impl PostgresGrantCategory {
    pub fn from_line(line: &str) -> Self {
        if line.starts_with("Role:") {
            Self::Role
        } else if line.starts_with("Attributes:") {
            Self::Attributes
        } else if line.starts_with("Member of:") {
            Self::MemberOf
        } else if line.starts_with("Has member:") {
            Self::HasMember
        } else if line.starts_with("Database:") {
            Self::Database
        } else if line.starts_with("Schema:") {
            Self::Schema
        } else if line.starts_with("Table:") {
            Self::Table
        } else {
            Self::Other
        }
    }
}

/// A single formatted grant line parsed from the PostgreSQL grants CTE.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PostgresGrantLine {
    pub category: PostgresGrantCategory,
    pub text: String,
}

impl PostgresGrantLine {
    pub fn parse(text: impl Into<String>) -> Self {
        let text = text.into();
        let category = PostgresGrantCategory::from_line(&text);
        Self { category, text }
    }
}

/// Target scope for granting or revoking PostgreSQL privileges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostgresPrivilegeScope {
    Database { database: String },
    Schema { schema: String },
    Table { schema: String, table: String },
    Role { role: String },
}

/// Escapes a PostgreSQL identifier safely inside double quotes.
pub fn quote_identifier(ident: &str) -> Result<String> {
    ensure!(
        !ident.is_empty(),
        "Identifier cannot be empty"
    );
    ensure!(
        ident.len() <= 256 && !ident.contains('\0'),
        "Invalid identifier"
    );
    Ok(format!("\"{}\"", ident.replace('"', "\"\"")))
}

/// Escapes a SQL string literal safely inside single quotes.
pub fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Returns the SQL to query all PostgreSQL roles and attributes.
pub fn list_roles_sql() -> &'static str {
    r#"SELECT
  r.rolname AS "user",
  CASE WHEN r.rolcanlogin THEN 'LOGIN' ELSE 'ROLE' END AS "host",
  concat_ws(', ',
    CASE WHEN r.rolsuper THEN 'SUPERUSER' END,
    CASE WHEN r.rolcreatedb THEN 'CREATEDB' END,
    CASE WHEN r.rolcreaterole THEN 'CREATEROLE' END,
    CASE WHEN r.rolreplication THEN 'REPLICATION' END,
    CASE WHEN row_to_json(r)::text LIKE '%"rolbypassrls":true%' THEN 'BYPASSRLS' END
  ) AS "plugin"
FROM pg_catalog.pg_roles r
ORDER BY r.rolname;"#
}

/// Builds the comprehensive CTE query that returns structured grants for a specific role.
pub fn show_grants_sql(role: &str) -> Result<String> {
    let quoted_role = quote_literal(role);
    Ok(format!(
        r#"WITH target AS (
  SELECT oid, rolname, rolsuper, rolcreatedb, rolcreaterole, rolcanlogin, rolreplication,
         row_to_json(r)::text LIKE '%"rolbypassrls":true%' AS rolbypassrls
  FROM pg_catalog.pg_roles r
  WHERE r.rolname = {quoted_role}
)
SELECT line
FROM (
  SELECT 1 AS sort, 'Role: ' || quote_ident(rolname) AS line
  FROM target
  UNION ALL
  SELECT 2, 'Attributes: ' || COALESCE(NULLIF(concat_ws(', ',
    CASE WHEN rolsuper THEN 'SUPERUSER' END,
    CASE WHEN rolcreatedb THEN 'CREATEDB' END,
    CASE WHEN rolcreaterole THEN 'CREATEROLE' END,
    CASE WHEN rolcanlogin THEN 'LOGIN' ELSE 'NOLOGIN' END,
    CASE WHEN rolreplication THEN 'REPLICATION' END,
    CASE WHEN rolbypassrls THEN 'BYPASSRLS' END
  ), ''), 'none')
  FROM target
  UNION ALL
  SELECT 10, 'Member of: ' || quote_ident(parent.rolname) || CASE WHEN m.admin_option THEN ' WITH ADMIN OPTION' ELSE '' END
  FROM pg_catalog.pg_auth_members m
  JOIN target t ON t.oid = m.member
  JOIN pg_catalog.pg_roles parent ON parent.oid = m.roleid
  UNION ALL
  SELECT 20, 'Has member: ' || quote_ident(member.rolname) || CASE WHEN m.admin_option THEN ' WITH ADMIN OPTION' ELSE '' END
  FROM pg_catalog.pg_auth_members m
  JOIN target t ON t.oid = m.roleid
  JOIN pg_catalog.pg_roles member ON member.oid = m.member
  UNION ALL
  SELECT 30, 'Database: ' || quote_ident(d.datname) || ' = ' ||
    concat_ws(', ',
      CASE WHEN has_database_privilege(t.rolname, d.oid, 'CONNECT') THEN 'CONNECT' END,
      CASE WHEN has_database_privilege(t.rolname, d.oid, 'CREATE') THEN 'CREATE' END,
      CASE WHEN has_database_privilege(t.rolname, d.oid, 'TEMPORARY') THEN 'TEMPORARY' END
    )
  FROM target t
  CROSS JOIN pg_catalog.pg_database d
  WHERE has_database_privilege(t.rolname, d.oid, 'CONNECT')
     OR has_database_privilege(t.rolname, d.oid, 'CREATE')
     OR has_database_privilege(t.rolname, d.oid, 'TEMPORARY')
  UNION ALL
  SELECT 40, 'Schema: ' || quote_ident(n.nspname) || ' = ' ||
    concat_ws(', ',
      CASE WHEN has_schema_privilege(t.rolname, n.oid, 'USAGE') THEN 'USAGE' END,
      CASE WHEN has_schema_privilege(t.rolname, n.oid, 'CREATE') THEN 'CREATE' END
    )
  FROM target t
  CROSS JOIN pg_catalog.pg_namespace n
  WHERE n.nspname NOT LIKE 'pg~_%' ESCAPE '~'
    AND n.nspname <> 'information_schema'
    AND (has_schema_privilege(t.rolname, n.oid, 'USAGE') OR has_schema_privilege(t.rolname, n.oid, 'CREATE'))
  UNION ALL
  SELECT 50, 'Table: ' || quote_ident(table_schema) || '.' || quote_ident(table_name) || ' = ' ||
    string_agg(privilege_type || CASE WHEN is_grantable = 'YES' THEN ' WITH GRANT OPTION' ELSE '' END, ', ' ORDER BY privilege_type)
  FROM information_schema.role_table_grants
  WHERE grantee = {quoted_role}
  GROUP BY table_schema, table_name
) grants
ORDER BY sort, line;"#
    ))
}

/// Generates DDL to create a new PostgreSQL role with an encrypted password.
pub fn create_role_sql(name: &str, password: &str, can_login: bool) -> Result<String> {
    let quoted_name = quote_identifier(name)?;
    let quoted_pw = quote_literal(password);
    let login = if can_login { "LOGIN" } else { "NOLOGIN" };
    Ok(format!("CREATE ROLE {quoted_name} {login} PASSWORD {quoted_pw};"))
}

/// Generates DDL to change a PostgreSQL role's password.
pub fn alter_role_password_sql(name: &str, password: &str) -> Result<String> {
    let quoted_name = quote_identifier(name)?;
    let quoted_pw = quote_literal(password);
    Ok(format!("ALTER ROLE {quoted_name} PASSWORD {quoted_pw};"))
}

/// Generates DDL to toggle a PostgreSQL role's login capability.
pub fn alter_role_login_sql(name: &str, enabled: bool) -> Result<String> {
    let quoted_name = quote_identifier(name)?;
    let login = if enabled { "LOGIN" } else { "NOLOGIN" };
    Ok(format!("ALTER ROLE {quoted_name} {login};"))
}

/// Generates DDL to drop a PostgreSQL role.
pub fn drop_role_sql(name: &str) -> Result<String> {
    let quoted_name = quote_identifier(name)?;
    Ok(format!("DROP ROLE {quoted_name};"))
}

/// Validates and formats the target SQL clause for a given privilege scope.
pub fn privilege_target_sql(scope: &PostgresPrivilegeScope) -> Result<String> {
    match scope {
        PostgresPrivilegeScope::Database { database } => {
            let quoted = quote_identifier(database)?;
            Ok(format!("DATABASE {quoted}"))
        }
        PostgresPrivilegeScope::Schema { schema } => {
            let quoted = quote_identifier(schema)?;
            Ok(format!("SCHEMA {quoted}"))
        }
        PostgresPrivilegeScope::Table { schema, table } => {
            let quoted_schema = quote_identifier(schema)?;
            if table.trim() == "*" {
                Ok(format!("ALL TABLES IN SCHEMA {quoted_schema}"))
            } else {
                let quoted_table = quote_identifier(table)?;
                Ok(format!("TABLE {quoted_schema}.{quoted_table}"))
            }
        }
        PostgresPrivilegeScope::Role { role } => quote_identifier(role),
    }
}

/// Generates a `GRANT` statement across any supported PostgreSQL scope.
pub fn grant_privileges_sql(
    role: &str,
    scope: &PostgresPrivilegeScope,
    privileges: &[&str],
    with_grant_option: bool,
) -> Result<String> {
    let quoted_role = quote_identifier(role)?;
    if let PostgresPrivilegeScope::Role { role: target_role } = scope {
        let quoted_target = quote_identifier(target_role)?;
        let admin_opt = if with_grant_option { " WITH ADMIN OPTION" } else { "" };
        return Ok(format!("GRANT {quoted_target} TO {quoted_role}{admin_opt};"));
    }
    ensure!(!privileges.is_empty(), "At least one privilege must be selected");
    let privs = privileges.join(", ");
    let target = privilege_target_sql(scope)?;
    let grant_opt = if with_grant_option { " WITH GRANT OPTION" } else { "" };
    Ok(format!("GRANT {privs} ON {target} TO {quoted_role}{grant_opt};"))
}

/// Generates a `REVOKE` statement across any supported PostgreSQL scope.
pub fn revoke_privileges_sql(
    role: &str,
    scope: &PostgresPrivilegeScope,
    privileges: &[&str],
) -> Result<String> {
    let quoted_role = quote_identifier(role)?;
    if let PostgresPrivilegeScope::Role { role: target_role } = scope {
        let quoted_target = quote_identifier(target_role)?;
        return Ok(format!("REVOKE {quoted_target} FROM {quoted_role};"));
    }
    ensure!(!privileges.is_empty(), "At least one privilege must be selected");
    let privs = privileges.join(", ");
    let target = privilege_target_sql(scope)?;
    Ok(format!("REVOKE {privs} ON {target} FROM {quoted_role};"))
}

/// Parses the output rows from `list_roles_sql()`.
pub fn parse_roles_rows(rows: &[tokio_postgres::Row]) -> Vec<PostgresRole> {
    rows.iter()
        .map(|row| {
            let name: String = row.get(0);
            let host: String = row.get(1);
            let plugin: String = row.get(2);
            let can_login = host == "LOGIN";
            let is_superuser = plugin.contains("SUPERUSER");
            let can_create_db = plugin.contains("CREATEDB");
            let can_create_role = plugin.contains("CREATEROLE");
            let replication = plugin.contains("REPLICATION");
            let bypass_rls = plugin.contains("BYPASSRLS");
            PostgresRole {
                name,
                can_login,
                is_superuser,
                can_create_db,
                can_create_role,
                replication,
                bypass_rls,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_identifier_escapes_double_quotes_and_rejects_empty() {
        assert_eq!(quote_identifier("admin").unwrap(), "\"admin\"");
        assert_eq!(quote_identifier("user\"name").unwrap(), "\"user\"\"name\"");
        assert!(quote_identifier("").is_err());
        assert!(quote_identifier("bad\0ident").is_err());
    }

    #[test]
    fn quote_literal_escapes_single_quotes() {
        assert_eq!(quote_literal("secret"), "'secret'");
        assert_eq!(quote_literal("p'ass''word"), "'p''ass''''word'");
    }

    #[test]
    fn creates_and_alters_roles_sql() {
        assert_eq!(
            create_role_sql("app_user", "sec'ret", true).unwrap(),
            "CREATE ROLE \"app_user\" LOGIN PASSWORD 'sec''ret';"
        );
        assert_eq!(
            create_role_sql("read_only", "pass", false).unwrap(),
            "CREATE ROLE \"read_only\" NOLOGIN PASSWORD 'pass';"
        );
        assert_eq!(
            alter_role_password_sql("app_user", "new_sec").unwrap(),
            "ALTER ROLE \"app_user\" PASSWORD 'new_sec';"
        );
        assert_eq!(
            alter_role_login_sql("app_user", false).unwrap(),
            "ALTER ROLE \"app_user\" NOLOGIN;"
        );
        assert_eq!(
            drop_role_sql("app_user").unwrap(),
            "DROP ROLE \"app_user\";"
        );
    }

    #[test]
    fn generates_grants_across_scopes() {
        // Database scope
        let db_scope = PostgresPrivilegeScope::Database {
            database: "production".into(),
        };
        assert_eq!(
            grant_privileges_sql("app_user", &db_scope, &["CONNECT", "TEMPORARY"], false).unwrap(),
            "GRANT CONNECT, TEMPORARY ON DATABASE \"production\" TO \"app_user\";"
        );
        assert_eq!(
            grant_privileges_sql("app_user", &db_scope, &["CONNECT"], true).unwrap(),
            "GRANT CONNECT ON DATABASE \"production\" TO \"app_user\" WITH GRANT OPTION;"
        );
        assert_eq!(
            revoke_privileges_sql("app_user", &db_scope, &["CONNECT"],).unwrap(),
            "REVOKE CONNECT ON DATABASE \"production\" FROM \"app_user\";"
        );

        // Schema scope
        let schema_scope = PostgresPrivilegeScope::Schema {
            schema: "analytics".into(),
        };
        assert_eq!(
            grant_privileges_sql("app_user", &schema_scope, &["USAGE"], false).unwrap(),
            "GRANT USAGE ON SCHEMA \"analytics\" TO \"app_user\";"
        );

        // Table scope single table
        let table_scope = PostgresPrivilegeScope::Table {
            schema: "public".into(),
            table: "orders".into(),
        };
        assert_eq!(
            grant_privileges_sql("app_user", &table_scope, &["SELECT", "INSERT"], false).unwrap(),
            "GRANT SELECT, INSERT ON TABLE \"public\".\"orders\" TO \"app_user\";"
        );

        // Table scope all tables in schema
        let all_tables_scope = PostgresPrivilegeScope::Table {
            schema: "public".into(),
            table: "*".into(),
        };
        assert_eq!(
            grant_privileges_sql("app_user", &all_tables_scope, &["SELECT"], false).unwrap(),
            "GRANT SELECT ON ALL TABLES IN SCHEMA \"public\" TO \"app_user\";"
        );

        // Role membership scope
        let role_scope = PostgresPrivilegeScope::Role {
            role: "reporting_group".into(),
        };
        assert_eq!(
            grant_privileges_sql("app_user", &role_scope, &[], true).unwrap(),
            "GRANT \"reporting_group\" TO \"app_user\" WITH ADMIN OPTION;"
        );
        assert_eq!(
            revoke_privileges_sql("app_user", &role_scope, &[]).unwrap(),
            "REVOKE \"reporting_group\" FROM \"app_user\";"
        );
    }

    #[test]
    fn parses_grant_lines_correctly() {
        let line = PostgresGrantLine::parse("Database: production = CONNECT, TEMPORARY");
        assert_eq!(line.category, PostgresGrantCategory::Database);

        let line = PostgresGrantLine::parse("Role: \"app_user\"");
        assert_eq!(line.category, PostgresGrantCategory::Role);

        let line = PostgresGrantLine::parse("Attributes: LOGIN, CREATEDB");
        assert_eq!(line.category, PostgresGrantCategory::Attributes);

        let line = PostgresGrantLine::parse("Member of: \"analysts\" WITH ADMIN OPTION");
        assert_eq!(line.category, PostgresGrantCategory::MemberOf);

        let line = PostgresGrantLine::parse("Table: public.orders = SELECT, INSERT");
        assert_eq!(line.category, PostgresGrantCategory::Table);
    }

    #[test]
    fn show_grants_sql_contains_expected_unions() {
        let sql = show_grants_sql("kaj-nashir-devstag").unwrap();
        assert!(sql.contains("WHERE r.rolname = 'kaj-nashir-devstag'"));
        assert!(sql.contains("has_database_privilege"));
        assert!(sql.contains("has_schema_privilege"));
        assert!(sql.contains("information_schema.role_table_grants"));
    }
}
