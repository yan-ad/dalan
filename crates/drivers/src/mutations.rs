//! Bounded, parameter-only staged table writes. No arbitrary SQL is accepted.
//! Callers must never stage originals from a truncated preview. Originals are a
//! complete row in metadata order; unsupported/lossy preview types fail closed.
use crate::{
    mysql::{CellValue, ColumnInfo},
    sources::{DbEngine, SourceProfile},
};
use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WriteRequest {
    pub database: String,
    pub table: String,
    /// Complete metadata in preview order; checked again under the table lock.
    pub expected_columns: Vec<ColumnInfo>,
    pub mutations: Vec<RowMutation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RowMutation {
    Update {
        original: Vec<CellValue>,
        changes: Vec<(usize, CellValue)>,
    },
    Delete {
        original: Vec<CellValue>,
    },
    Insert {
        values: Vec<(usize, CellValue)>,
    },
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteReport {
    pub updated: u64,
    pub inserted: u64,
    pub deleted: u64,
}
/// A failed COMMIT acknowledgement is not evidence of rollback. Never retry it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteFailure {
    CommitUncertain,
}
impl fmt::Display for WriteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Write outcome unknown: commit acknowledgement failed; do not retry; reload and verify the table")
    }
}
impl std::error::Error for WriteFailure {}

pub async fn apply_table_changes(
    profile: &SourceProfile,
    password: &str,
    request: &WriteRequest,
) -> Result<WriteReport> {
    validate_request(request)?;
    crate::versions::selected(profile.engine)?;
    match profile.engine {
        DbEngine::MySql | DbEngine::MariaDb => {
            crate::mysql::apply_table_changes(profile, password, request).await
        }
        DbEngine::PostgreSql => {
            crate::postgres::apply_table_changes(profile, password, request).await
        }
        _ => Err(anyhow!("Staged writes are not supported by this driver")),
    }
}

pub fn validate_request(r: &WriteRequest) -> Result<()> {
    ensure!(
        !r.database.is_empty()
            && r.database.len() <= 256
            && !r.database.contains('\0')
            && !r.table.is_empty()
            && r.table.len() <= 1024
            && !r.table.contains('\0'),
        "Invalid write table identifier"
    );
    ensure!(
        (1..=100).contains(&r.mutations.len()),
        "Write batch must contain 1 through 100 mutations"
    );
    ensure!(
        (1..=512).contains(&r.expected_columns.len()),
        "Write schema must contain 1 through 512 columns"
    );
    let mut names = HashSet::new();
    for c in &r.expected_columns {
        ensure!(
            !c.name.is_empty()
                && c.name.len() <= 1024
                && !c.name.contains('\0')
                && !c.data_type.is_empty()
                && c.data_type.len() <= 1024
                && !c.data_type.contains('\0')
                && names.insert(&c.name),
            "Invalid expected write column metadata"
        );
    }
    let mut bytes = 0usize;
    let mut cell = |v: &CellValue| -> Result<()> {
        let size = match v {
            CellValue::Null => 0,
            CellValue::Text(s) | CellValue::Number(s) => {
                ensure!(!s.contains('\0'), "Write values cannot contain NUL");
                s.len()
            }
            _ => return Err(anyhow!("Binary and temporal writes are not supported")),
        };
        ensure!(size <= 65536, "Write value exceeds 64 KiB");
        bytes += size;
        ensure!(bytes <= 2 * 1024 * 1024, "Write batch exceeds 2 MiB");
        Ok(())
    };
    for m in &r.mutations {
        let (original, changes) = match m {
            RowMutation::Update { original, changes } => (Some(original), Some(changes)),
            RowMutation::Delete { original } => (Some(original), None),
            RowMutation::Insert { values } => (None, Some(values)),
        };
        if let Some(original) = original {
            ensure!(
                original.len() == r.expected_columns.len(),
                "Invalid original row width"
            );
            for v in original {
                cell(v)?;
            }
        }
        if let Some(changes) = changes {
            ensure!(
                (1..=512).contains(&changes.len()),
                "Write requires 1 through 512 supplied columns"
            );
            let mut seen = HashSet::new();
            for (i, v) in changes {
                ensure!(
                    *i < r.expected_columns.len() && seen.insert(*i),
                    "Duplicate or out-of-range write column"
                );
                cell(v)?;
            }
        }
    }
    Ok(())
}

/// Compare every fingerprint field, including order, without normalizing names
/// or types. Call only after freshly reading metadata under the backend lock.
pub(crate) fn validate_schema(expected: &[ColumnInfo], actual: &[ColumnInfo]) -> Result<()> {
    ensure!(
        expected.len() == actual.len()
            && expected.iter().zip(actual).all(|(a, b)| {
                a.name == b.name
                    && a.data_type == b.data_type
                    && a.nullable == b.nullable
                    && a.is_primary_key == b.is_primary_key
            }),
        "Table schema changed since preview; reload before writing"
    );
    Ok(())
}
#[derive(Clone, Copy)]
pub(crate) enum Dialect {
    MySql,
    Postgres,
}
#[derive(Debug)]
pub(crate) struct Statement {
    pub sql: String,
    pub params: Vec<Option<String>>,
}
fn decimal(s: &str, integer: bool) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let mut parts = s.split('.');
    let a = parts.next().unwrap_or("");
    let b = parts.next();
    !a.is_empty()
        && a.bytes().all(|c| c.is_ascii_digit())
        && parts.next().is_none()
        && match b {
            None => true,
            Some(b) => !integer && !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()),
        }
}
fn kind(c: &ColumnInfo, d: Dialect) -> Result<&'static str> {
    let t = c.data_type.as_str();
    let pg = matches!(d, Dialect::Postgres);
    let k = if pg {
        match t {
            "smallint" => "int2",
            "integer" => "int4",
            "bigint" => "int8",
            "boolean" => "bool",
            "text" => "text",
            "character varying" => "varchar",
            "character" => "bpchar",
            _ if modifier(t, "character varying") => "varchar",
            _ if modifier(t, "character") => "bpchar",
            _ => {
                return Err(anyhow!(
                    "Table contains unsupported or lossy write column types"
                ));
            }
        }
    } else {
        match t {
            "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" => "int8",
            "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" => "text",
            _ => {
                return Err(anyhow!(
                    "Table contains unsupported or lossy write column types"
                ));
            }
        }
    };
    Ok(k)
}
fn modifier(t: &str, prefix: &str) -> bool {
    t.strip_prefix(prefix)
        .and_then(|s| s.strip_prefix('('))
        .and_then(|s| s.strip_suffix(')'))
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit() || b == b','))
}
fn value(v: &CellValue, c: &ColumnInfo, d: Dialect) -> Result<Option<String>> {
    let k = kind(c, d)?;
    match v {
        CellValue::Null => {
            ensure!(c.nullable, "NULL is not allowed in this write column");
            Ok(None)
        }
        CellValue::Text(s) if k == "text" || k == "varchar" || k == "bpchar" => Ok(Some(s.clone())),
        CellValue::Text(s) if k == "bool" && matches!(s.as_str(), "true" | "false") => {
            Ok(Some(s.clone()))
        }
        CellValue::Number(s) if decimal(s, true) && matches!(k, "int2" | "int4" | "int8") => {
            Ok(Some(s.clone()))
        }
        _ => Err(anyhow!(
            "Write value does not match the supported column type"
        )),
    }
}
/// Identifiers must already be quoted by the native backend, never request SQL.
pub(crate) fn compile(
    m: &RowMutation,
    columns: &[ColumnInfo],
    table: &str,
    names: &[String],
    d: Dialect,
) -> Result<Statement> {
    ensure!(
        !columns.is_empty() && columns.len() <= 512 && names.len() == columns.len(),
        "Invalid write metadata"
    );
    ensure!(
        columns.iter().any(|c| c.is_primary_key)
            && columns
                .iter()
                .filter(|c| c.is_primary_key)
                .all(|c| !c.nullable),
        "Writes require a complete non-nullable primary key"
    );
    for c in columns {
        kind(c, d)?;
    }
    let mut params = Vec::new();
    let mut bind = |i: usize, v: &CellValue| -> Result<String> {
        ensure!(
            i < columns.len(),
            "Write column is not in current table metadata"
        );
        params.push(value(v, &columns[i], d)?);
        Ok(match d {
            Dialect::Postgres => format!(
                "${}::text::pg_catalog.{}",
                params.len(),
                kind(&columns[i], d)?
            ),
            Dialect::MySql => "?".into(),
        })
    };
    let (original, changes, verb) = match m {
        RowMutation::Update { original, changes } => (Some(original), Some(changes), "UPDATE"),
        RowMutation::Delete { original } => (Some(original), None, "DELETE FROM"),
        RowMutation::Insert { values } => (None, Some(values), "INSERT INTO"),
    };
    let mut sql = format!("{verb} {table}");
    if let Some(changes) = changes {
        ensure!(!changes.is_empty(), "Empty write changes");
        let mut seen = HashSet::new();
        let mut assignments = Vec::new();
        let mut insert_names = Vec::new();
        for (i, v) in changes {
            ensure!(
                *i < columns.len() && seen.insert(*i),
                "Duplicate or out-of-range write column"
            );
            if original.is_some() {
                ensure!(
                    !columns[*i].is_primary_key,
                    "Changing primary key columns is not supported"
                );
            }
            let p = bind(*i, v)?;
            insert_names.push(names[*i].clone());
            assignments.push(if original.is_some() {
                format!("{}={p}", names[*i])
            } else {
                p
            });
        }
        if original.is_some() {
            sql.push_str(&format!(" SET {}", assignments.join(",")));
        } else {
            sql.push_str(&format!(
                " ({}) VALUES ({})",
                insert_names.join(","),
                assignments.join(",")
            ));
        }
    }
    if let Some(original) = original {
        ensure!(
            original.len() == columns.len(),
            "Original row no longer matches table metadata"
        );
        let mut predicates = Vec::new();
        for (i, v) in original.iter().enumerate() {
            let p = bind(i, v)?;
            predicates.push(match d {
                Dialect::Postgres
                    if matches!(kind(&columns[i], d)?, "text" | "varchar" | "bpchar") =>
                {
                    format!(
                        "{}::text COLLATE \"C\" IS NOT DISTINCT FROM ({p})::text COLLATE \"C\"",
                        names[i]
                    )
                }
                Dialect::Postgres => format!("{} IS NOT DISTINCT FROM {p}", names[i]),
                Dialect::MySql if kind(&columns[i], d)? == "text" => {
                    format!("BINARY {} <=> BINARY {p}", names[i])
                }
                Dialect::MySql => format!("BINARY CAST({} AS CHAR) <=> BINARY {p}", names[i]),
            });
        }
        sql.push_str(&format!(" WHERE {}", predicates.join(" AND ")));
    }
    Ok(Statement { sql, params })
}
pub(crate) fn count(report: &mut WriteReport, mutation: &RowMutation, affected: u64) -> Result<()> {
    ensure!(
        affected == 1,
        "Write conflict: expected exactly one affected row; batch was not committed"
    );
    match mutation {
        RowMutation::Update { .. } => report.updated += 1,
        RowMutation::Insert { .. } => report.inserted += 1,
        RowMutation::Delete { .. } => report.deleted += 1,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cols() -> Vec<ColumnInfo> {
        vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "integer".into(),
                nullable: false,
                is_primary_key: true,
            },
            ColumnInfo {
                name: "a\"b".into(),
                data_type: "text".into(),
                nullable: true,
                is_primary_key: false,
            },
        ]
    }
    #[test]
    fn schema_fingerprint_requires_exact_fields_and_order() {
        let expected = cols();
        validate_schema(&expected, &expected).unwrap();
        assert!(validate_schema(&expected, &expected[..1]).is_err());
        let mut actual = expected.clone();
        actual.reverse();
        assert!(validate_schema(&expected, &actual).is_err());
        for field in 0..4 {
            let mut actual = expected.clone();
            match field {
                0 => actual[1].name = "A\"b".into(),
                1 => actual[1].data_type = "varchar".into(),
                2 => actual[1].nullable = false,
                _ => actual[1].is_primary_key = true,
            }
            assert!(validate_schema(&expected, &actual).is_err());
        }
    }
    #[test]
    fn request_rejects_invalid_schema_and_schema_relative_indices() {
        let mut r = WriteRequest {
            database: "d".into(),
            table: "t".into(),
            expected_columns: cols(),
            mutations: vec![RowMutation::Insert {
                values: vec![(1, CellValue::Text("ok".into()))],
            }],
        };
        validate_request(&r).unwrap();
        r.expected_columns.clear();
        assert!(validate_request(&r).is_err());
        r.expected_columns = vec![cols()[0].clone(); 513];
        assert!(validate_request(&r).is_err());
        r.expected_columns = cols();
        r.expected_columns[1].name = r.expected_columns[0].name.clone();
        assert!(validate_request(&r).is_err());
        r.expected_columns = cols();
        r.mutations = vec![RowMutation::Insert {
            values: vec![(2, CellValue::Null)],
        }];
        assert!(validate_request(&r).is_err());
    }
    #[test]
    fn numeric_tables_fail_closed_even_when_numeric_column_is_untouched() {
        let m = RowMutation::Insert {
            values: vec![(0, CellValue::Number("1".into()))],
        };
        for d in [Dialect::MySql, Dialect::Postgres] {
            for ty in ["numeric", "decimal", "numeric(5,2)", "decimal(5,2)"] {
                let mut c = cols();
                c[1].data_type = ty.into();
                assert!(compile(&m, &c, "t", &["id".into(), "a".into()], d).is_err());
            }
        }
    }
    #[test]
    fn bound_full_original_predicate() {
        let m = RowMutation::Update {
            original: vec![CellValue::Number("1".into()), CellValue::Null],
            changes: vec![(1, CellValue::Text("'; DROP TABLE x".into()))],
        };
        let s = compile(
            &m,
            &cols(),
            "\"s\".\"t\"",
            &["\"id\"".into(), "\"a\"\"b\"".into()],
            Dialect::Postgres,
        )
        .unwrap();
        assert_eq!(
            s.sql,
            "UPDATE \"s\".\"t\" SET \"a\"\"b\"=$1::text::pg_catalog.text WHERE \"id\" IS NOT DISTINCT FROM $2::text::pg_catalog.int4 AND \"a\"\"b\"::text COLLATE \"C\" IS NOT DISTINCT FROM ($3::text::pg_catalog.text)::text COLLATE \"C\""
        );
        assert_eq!(s.params.len(), 3);
        assert!(!s.sql.contains("DROP"));
    }
    #[test]
    fn malformed_and_lossy_fail_closed() {
        let mut r = WriteRequest {
            database: "d".into(),
            table: "t".into(),
            expected_columns: cols(),
            mutations: vec![RowMutation::Insert {
                values: vec![(0, CellValue::Number("1".into())), (0, CellValue::Null)],
            }],
        };
        assert!(validate_request(&r).is_err());
        r.mutations = vec![RowMutation::Delete {
            original: vec![CellValue::Binary("0x01".into())],
        }];
        assert!(validate_request(&r).is_err());
        assert!(!decimal("1e4", false));
        assert!(!decimal("1;select", false));
        let mut c = cols();
        c[1].data_type = "timestamp".into();
        assert!(kind(&c[1], Dialect::Postgres).is_err());
        c[1].data_type = "numeric(5,2);DELETE".into();
        assert!(kind(&c[1], Dialect::Postgres).is_err());
    }
    #[test]
    fn bounds_and_key_shape_are_checked() {
        let m = RowMutation::Insert {
            values: vec![(1, CellValue::Text("x".repeat(65537)))],
        };
        let mut r = WriteRequest {
            database: "d".into(),
            table: "t".into(),
            mutations: vec![m],
            expected_columns: cols(),
        };
        assert!(validate_request(&r).is_err());
        let m = RowMutation::Delete {
            original: vec![CellValue::Number("1".into()), CellValue::Null],
        };
        r.mutations = vec![m.clone(); 101];
        assert!(validate_request(&r).is_err());
        r.mutations = vec![
            RowMutation::Insert {
                values: vec![(1, CellValue::Text("x".repeat(65536)))]
            };
            33
        ];
        assert!(validate_request(&r).is_err());
        let names = vec!["`id`".into(), "`a`".into()];
        let mut c = cols();
        c[0].is_primary_key = false;
        assert!(compile(&m, &c, "`d`.`t`", &names, Dialect::MySql).is_err());
        c[0].is_primary_key = true;
        c[0].nullable = true;
        assert!(compile(&m, &c, "`d`.`t`", &names, Dialect::MySql).is_err());
        c[0].nullable = false;
        let bad = RowMutation::Delete {
            original: vec![CellValue::Number("1".into())],
        };
        assert!(compile(&bad, &c, "`d`.`t`", &names, Dialect::MySql).is_err());
        let pk = RowMutation::Update {
            original: vec![CellValue::Number("1".into()), CellValue::Null],
            changes: vec![(0, CellValue::Number("2".into()))],
        };
        assert!(compile(&pk, &c, "`d`.`t`", &names, Dialect::MySql).is_err());
        let s = compile(&m, &c, "`d`.`t`", &names, Dialect::MySql).unwrap();
        assert!(s.sql.contains("BINARY CAST(`id` AS CHAR) <=> BINARY ?"));
        assert!(s.sql.contains("BINARY `a` <=> BINARY ?"));
    }
    #[test]
    fn conflict_never_counts_success() {
        let mut r = WriteReport::default();
        let m = RowMutation::Delete { original: vec![] };
        assert!(count(&mut r, &m, 0).is_err());
        assert!(count(&mut r, &m, 2).is_err());
        assert_eq!(r, WriteReport::default());
        count(&mut r, &m, 1).unwrap();
        assert_eq!(r.deleted, 1);
        assert!(
            WriteFailure::CommitUncertain
                .to_string()
                .contains("do not retry")
        );
    }
}
