//! Small, bounded table-browser grammar. Input SQL is never sent to the server:
//! identifiers are resolved against metadata and literals become prepared parameters.
use crate::{ColumnInfo, SortDirection};
use anyhow::{Result, anyhow, ensure};
use mysql_async::Value;
use sqlparser::{
    ast::{
        BinaryOperator, Expr, OrderByKind, SetExpr, Statement, UnaryOperator, Value as SqlValue,
    },
    dialect::MySqlDialect,
    keywords::Keyword,
    parser::Parser,
    tokenizer::{Token, Tokenizer, Whitespace},
};

pub(crate) struct CompiledTableClauses {
    pub(crate) where_sql: Option<String>,
    pub(crate) order_sql: Vec<(String, SortDirection)>,
    pub(crate) params: Vec<Value>,
}

/// Validate editable WHERE and ORDER BY fragments without opening a connection.
/// Use column names (backtick quoted if needed), comparisons, AND/OR/NOT,
/// LIKE, BETWEEN, IN, and IS [NOT] NULL. ORDER BY accepts at most eight
/// distinct columns with optional ASC/DESC. Functions and subqueries are forbidden.
/// Errors deliberately do not contain input SQL or parser diagnostics.
pub fn validate_table_clauses(
    where_clause: &str,
    order_by: &str,
    columns: &[ColumnInfo],
) -> Result<()> {
    compile(where_clause, order_by, Some(columns)).map(|_| ())
}

fn budget(fragment: &str) -> Result<()> {
    ensure!(fragment.len() <= 16 * 1024, "Table clause exceeds 16 KiB");
    let tokens = Tokenizer::new(&MySqlDialect {}, fragment)
        .tokenize()
        .map_err(|_| anyhow!("Invalid table clause syntax"))?;
    let (mut count, mut depth, mut cases, mut operators) = (0, 0usize, 0usize, 0);
    for token in tokens {
        match token {
            Token::Whitespace(Whitespace::Space | Whitespace::Newline | Whitespace::Tab) => {
                continue;
            }
            Token::Whitespace(_) => {
                return Err(anyhow!("Comments are not allowed in table clauses"));
            }
            Token::SemiColon => return Err(anyhow!("Statements are not allowed in table clauses")),
            Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
            Token::RParen | Token::RBracket | Token::RBrace => depth = depth.saturating_sub(1),
            Token::Word(ref w) => {
                if w.keyword == Keyword::CASE {
                    cases += 1;
                }
                if w.keyword == Keyword::END {
                    cases = cases.saturating_sub(1);
                }
                // Count every unquoted keyword, including unary chains and
                // constructs we will reject, before the recursive parser runs.
                if w.keyword != Keyword::NoKeyword {
                    operators += 1;
                }
            }
            Token::Number(..)
            | Token::Comma
            | Token::Period
            | Token::SingleQuotedString(_)
            | Token::DoubleQuotedString(_) => (),
            _ => operators += 1,
        }
        count += 1;
        ensure!(count <= 1024, "Table clause exceeds token complexity limit");
        ensure!(
            depth + cases <= 24,
            "Table clause exceeds nesting complexity limit"
        );
        ensure!(
            operators <= 128,
            "Table clause exceeds operator complexity limit"
        );
    }
    Ok(())
}

fn column(expr: &Expr, columns: Option<&[ColumnInfo]>) -> Result<String> {
    let Expr::Identifier(id) = expr else {
        return Err(anyhow!("Use an unqualified table column name"));
    };
    ensure!(
        columns.is_none_or(|cols| cols.iter().any(|c| c.name == id.value)),
        "Clause column is not in table metadata"
    );
    Ok(id.value.clone())
}
fn quote(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}
fn number(text: &str) -> Value {
    if let Ok(v) = text.parse::<i64>() {
        return Value::Int(v);
    }
    if let Ok(v) = text.parse::<u64>() {
        return Value::UInt(v);
    }
    // Decimal/exponent and out-of-range integers retain their exact spelling.
    // Binding bytes lets MySQL coerce them against the compared column type,
    // without a lossy f64 round trip.
    Value::from(text)
}
fn literal(expr: &Expr) -> Result<Value> {
    match expr {
        Expr::Nested(e) => literal(e),
        Expr::Value(v) => match &v.value {
            SqlValue::SingleQuotedString(s) | SqlValue::DoubleQuotedString(s) => {
                Ok(Value::from(s.clone()))
            }
            SqlValue::Number(s, false) => Ok(number(s)),
            SqlValue::Boolean(b) => Ok(Value::Int(i64::from(*b))),
            SqlValue::Null => Ok(Value::NULL),
            _ => Err(anyhow!("Use a string, number, boolean, or NULL literal")),
        },
        Expr::UnaryOp { op, expr } if matches!(op, UnaryOperator::Minus | UnaryOperator::Plus) => {
            if let Expr::Value(v) = expr.as_ref()
                && let SqlValue::Number(s, false) = &v.value
            {
                return Ok(number(&format!(
                    "{}{s}",
                    if *op == UnaryOperator::Minus { "-" } else { "" }
                )));
            }
            Err(anyhow!("Signs are allowed only on numeric literals"))
        }
        _ => Err(anyhow!(
            "Use a literal value; functions and subqueries are not allowed"
        )),
    }
}

fn expression(
    expr: &Expr,
    columns: Option<&[ColumnInfo]>,
    params: &mut Vec<Value>,
) -> Result<String> {
    let bind = |e: &Expr, p: &mut Vec<Value>| -> Result<String> {
        p.push(literal(e)?);
        Ok("?".into())
    };
    let col = |e: &Expr| column(e, columns).map(|n| quote(&n));
    match expr {
        Expr::Nested(e) => Ok(format!("({})", expression(e, columns, params)?)),
        Expr::Identifier(_) => col(expr),
        Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr,
        } => Ok(format!("(NOT {})", expression(expr, columns, params)?)),
        Expr::BinaryOp { left, op, right } => {
            let operator = match op {
                BinaryOperator::And => "AND",
                BinaryOperator::Or => "OR",
                BinaryOperator::Eq => "=",
                BinaryOperator::NotEq => "<>",
                BinaryOperator::Lt => "<",
                BinaryOperator::LtEq => "<=",
                BinaryOperator::Gt => ">",
                BinaryOperator::GtEq => ">=",
                _ => return Err(anyhow!("Only comparisons and AND/OR are allowed")),
            };
            let (left, right) = if matches!(op, BinaryOperator::And | BinaryOperator::Or) {
                (
                    expression(left, columns, params)?,
                    expression(right, columns, params)?,
                )
            } else if matches!(left.as_ref(), Expr::Identifier(_)) {
                (col(left)?, bind(right, params)?)
            } else {
                (bind(left, params)?, col(right)?)
            };
            Ok(format!("({left} {operator} {right})"))
        }
        Expr::IsNull(e) => Ok(format!("({} IS NULL)", col(e)?)),
        Expr::IsNotNull(e) => Ok(format!("({} IS NOT NULL)", col(e)?)),
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => Ok(format!(
            "({} {}BETWEEN {} AND {})",
            col(expr)?,
            if *negated { "NOT " } else { "" },
            bind(low, params)?,
            bind(high, params)?
        )),
        Expr::InList {
            expr,
            list,
            negated,
        } => {
            ensure!(
                !list.is_empty() && list.len() <= 256,
                "IN requires 1 through 256 literal values"
            );
            let name = col(expr)?;
            let values = list
                .iter()
                .map(|e| bind(e, params))
                .collect::<Result<Vec<_>>>()?;
            Ok(format!(
                "({name} {}IN ({}))",
                if *negated { "NOT " } else { "" },
                values.join(",")
            ))
        }
        Expr::Like {
            expr,
            pattern,
            negated,
            any: false,
            escape_char: None,
        } => Ok(format!(
            "({} {}LIKE {})",
            col(expr)?,
            if *negated { "NOT " } else { "" },
            bind(pattern, params)?
        )),
        _ => Err(anyhow!(
            "Unsupported WHERE expression; use column comparisons, LIKE, IN, BETWEEN, or IS NULL"
        )),
    }
}

pub(crate) fn compile(
    where_clause: &str,
    order_by: &str,
    columns: Option<&[ColumnInfo]>,
) -> Result<CompiledTableClauses> {
    let mut compiled = CompiledTableClauses {
        where_sql: None,
        order_sql: Vec::new(),
        params: Vec::new(),
    };
    for (fragment, is_where) in [(where_clause, true), (order_by, false)] {
        budget(fragment)?;
        if fragment.trim().is_empty() {
            continue;
        }
        let prefix = if is_where { "WHERE" } else { "ORDER BY" };
        let sql = format!("SELECT * FROM `__dalan__` {prefix} {fragment}");
        let mut statements = Parser::parse_sql(&MySqlDialect {}, &sql)
            .map_err(|_| anyhow!("Invalid table clause syntax; use e.g. id >= 10 or name DESC"))?;
        ensure!(statements.len() == 1, "Only one table clause is allowed");
        let Statement::Query(query) = statements.remove(0) else {
            return Err(anyhow!("Expected a table clause"));
        };
        let mut query = *query;
        let SetExpr::Select(select) = query.body.as_mut() else {
            return Err(anyhow!("Subqueries and set operations are not allowed"));
        };
        if is_where {
            let expr = select
                .selection
                .take()
                .ok_or_else(|| anyhow!("Expected a WHERE expression"))?;
            compiled.where_sql = Some(expression(&expr, columns, &mut compiled.params)?);
        } else {
            let order = query
                .order_by
                .take()
                .ok_or_else(|| anyhow!("Expected column ordering"))?;
            ensure!(
                order.interpolate.is_none(),
                "ORDER BY modifiers are not allowed"
            );
            let OrderByKind::Expressions(items) = order.kind else {
                return Err(anyhow!("ORDER BY requires column names"));
            };
            ensure!(
                !items.is_empty() && items.len() <= 8,
                "ORDER BY requires 1 through 8 columns"
            );
            for item in items {
                ensure!(
                    item.options.nulls_first.is_none() && item.with_fill.is_none(),
                    "ORDER BY supports only ASC or DESC"
                );
                let name = column(&item.expr, columns)?;
                ensure!(
                    !compiled.order_sql.iter().any(|(n, _)| n == &name),
                    "ORDER BY columns must be unique"
                );
                compiled.order_sql.push((
                    name,
                    if item.options.asc == Some(false) {
                        SortDirection::Descending
                    } else {
                        SortDirection::Ascending
                    },
                ));
            }
        }
        // Removing the sole permitted clause must recover exactly our fixed
        // wrapper. This rejects LIMIT, locks, INTO, joins, GROUP BY, UNION, etc.
        ensure!(
            query.to_string() == "SELECT * FROM `__dalan__`",
            "Additional SQL clauses are not allowed"
        );
    }
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn columns() -> Vec<ColumnInfo> {
        ["id", "name", "a`b", "日本語"]
            .into_iter()
            .map(|name| ColumnInfo {
                name: name.into(),
                data_type: "varchar".into(),
                nullable: true,
                is_primary_key: name == "id",
            })
            .collect()
    }
    #[test]
    fn compiles_only_metadata_names_and_bound_literals() {
        let c = compile("NOT (id BETWEEN -2 AND 18446744073709551615) OR (`a``b` IN ('x; /* not a comment */', NULL, true) AND name NOT LIKE '%foo_')", "`日本語` DESC, id ASC", Some(&columns())).unwrap();
        assert_eq!(
            c.where_sql.as_deref(),
            Some(
                "((NOT ((`id` BETWEEN ? AND ?))) OR (((`a``b` IN (?,?,?)) AND (`name` NOT LIKE ?))))"
            )
        );
        assert_eq!(
            c.params,
            vec![
                Value::Int(-2),
                Value::UInt(u64::MAX),
                Value::from("x; /* not a comment */"),
                Value::NULL,
                Value::Int(1),
                Value::from("%foo_")
            ]
        );
        assert_eq!(
            c.order_sql,
            vec![
                ("日本語".into(), SortDirection::Descending),
                ("id".into(), SortDirection::Ascending)
            ]
        );
        for fragment in [
            "id = 12.345678901234567890",
            "id = 1e-100",
            "id = +18446744073709551616",
        ] {
            let c = compile(fragment, "", Some(&columns())).unwrap();
            assert!(matches!(c.params[0], Value::Bytes(_)));
        }
        let c = compile("name = 'it''s a secret'", "", Some(&columns())).unwrap();
        assert!(!c.where_sql.unwrap().contains("secret"));
        assert_eq!(c.params, vec![Value::from("it's a secret")]);
        for fragment in [
            "id != 1",
            "id <= 1",
            "id >= 1",
            "1 < id",
            "id IS NULL",
            "id IS NOT NULL",
            "id NOT IN (1,2)",
        ] {
            validate_table_clauses(fragment, "", &columns()).unwrap();
        }
    }
    #[test]
    fn rejects_sql_outside_the_browser_grammar() {
        for fragment in [
            "id = 1;",
            "id = 1; DELETE FROM contact",
            "id = 1 -- hi",
            "id = 1 # hi",
            "id = 1 /* comment */",
            "id = 1 /*!50000 OR 1=1 */",
            "id = 1 /*M! OR 1=1 */",
            "id = 1 /*+ MAX_EXECUTION_TIME(999999) */",
            "id = 1 UNION SELECT 2",
            "id = 1 ORDER BY id",
            "id = 1 LIMIT 1",
            "id = 1 FOR UPDATE",
            "id = 1 INTO OUTFILE 'secret'",
            "id = 1 GROUP BY id",
            "id IN (SELECT id FROM contact)",
            "SLEEP(1)",
            "name = LOAD_FILE('secret')",
            "unknown = 1",
            "t.id = 1",
            "@id = 1",
            "id = ?",
            "id + 1 = 2",
            "CAST(id AS CHAR) = '1'",
            "id = name",
            "id = (SELECT 1)",
            "name COLLATE utf8_bin = 'x'",
            "name REGEXP 'x'",
            "name LIKE '%x' ESCAPE '!'",
        ] {
            let error = validate_table_clauses(fragment, "", &columns()).unwrap_err();
            assert!(
                !error.to_string().contains("secret"),
                "error leaked literal"
            );
        }
        for fragment in [
            "1",
            "'name'",
            "LOWER(name)",
            "SLEEP(1)",
            "t.id",
            "id NULLS FIRST",
            "id,id",
            "id LIMIT 1",
            "id FOR UPDATE",
            "unknown",
            "id;",
            "id DESC /*hint*/",
            "id COLLATE utf8_bin",
        ] {
            assert!(
                validate_table_clauses("", fragment, &columns()).is_err(),
                "accepted {fragment}"
            );
        }
        validate_table_clauses(" \n\t", "", &columns()).unwrap();
    }
    #[test]
    fn rejects_complexity_before_recursive_parse() {
        for fragment in [
            " ".repeat(16 * 1024 + 1),
            format!("{}id{}", "(".repeat(25), ")".repeat(25)),
            format!("{}id", "NOT ".repeat(129)),
            format!("id = {}", "1+".repeat(2000)),
            format!(
                "{}1{}",
                "CASE WHEN true THEN ".repeat(25),
                " END".repeat(25)
            ),
            format!("id IN ({})", vec!["1"; 257].join(",")),
            format!("id IN ({})", vec!["1"; 1025].join(",")),
        ] {
            assert!(validate_table_clauses(&fragment, "", &columns()).is_err());
        }
        let cols = (0..9)
            .map(|i| ColumnInfo {
                name: format!("c{i}"),
                data_type: "int".into(),
                nullable: false,
                is_primary_key: false,
            })
            .collect::<Vec<_>>();
        validate_table_clauses(
            "",
            &cols[..8]
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(","),
            &cols,
        )
        .unwrap();
        assert!(
            validate_table_clauses(
                "",
                &cols
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                &cols
            )
            .is_err()
        );
        let c = compile(
            &format!("id IN ({})", vec!["1"; 256].join(",")),
            "",
            Some(&columns()),
        )
        .unwrap();
        assert_eq!(c.params.len(), 256);
    }
}
