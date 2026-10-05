//! Initial query-console backend: one parsed SELECT/CTE/UNION, never rewritten.
//!
//! This is defense in depth, not a replacement for a least-privilege SELECT-only
//! account. Views and server-defined objects can execute code outside this AST;
//! use trusted schemas. Every run owns a fresh read-only transaction and session.
//! Dropping the future cancels local work and disposes the connection/relay;
//! server execution is additionally limited to 20 seconds (not an immediate KILL).
use crate::{
    DbEngine, SourceProfile, TablePage,
    mysql::{self, ColumnInfo, Preview, Session},
};
use anyhow::{Result, anyhow, ensure};
use mysql_async::{
    consts::{ColumnFlags, ColumnType},
    prelude::Queryable,
};
use serde::{Deserialize, Serialize};
use sqlparser::{
    ast::{BinaryOperator, Expr, Query, Select, SetExpr, Statement, TableFactor, Visit, Visitor},
    dialect::MySqlDialect,
    keywords::Keyword,
    parser::Parser,
    tokenizer::{Token, Tokenizer},
};
use std::{ops::ControlFlow, time::Instant};

const SQL_CAP: usize = 64 * 1024;
const TOKEN_CAP: usize = 4096;
const NESTING_CAP: usize = 32;
const OPERATOR_CAP: usize = 256;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
    pub limit: u32,
}
impl Default for QueryRequest {
    fn default() -> Self {
        Self {
            sql: String::new(),
            limit: 100,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub page: TablePage,
    pub elapsed_ms: u64,
    pub warnings: Vec<String>,
}

// MySQL executable comments are invisible to a conventional SQL AST. Track
// literal/identifier/comment context, including doubled quotes and backslashes.
// Optimizer hints are also forbidden: they can override execution-time settings.
fn check_comments(sql: &str) -> Result<()> {
    let b = sql.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            q @ (b'\'' | b'"' | b'`') => {
                i += 1;
                while i < b.len() {
                    if b[i] == b'\\' && q != b'`' {
                        i = (i + 2).min(b.len());
                    } else if b[i] == q {
                        i += 1;
                        if i < b.len() && b[i] == q {
                            i += 1;
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'-' if b.get(i + 1) == Some(&b'-') && b.get(i + 2).is_none_or(|c| *c <= b' ') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'-' if b.get(i + 1) == Some(&b'-') => {
                // sqlparser treats every -- as a comment; MySQL requires
                // whitespace/control after it. Otherwise 1--SLEEP(1) is an
                // expression on the server but invisible in the parsed AST.
                return Err(anyhow!("Double-dash comments require following whitespace"));
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                ensure!(
                    b.get(i + 2) != Some(&b'!')
                        && b.get(i + 2) != Some(&b'+')
                        && !(b.get(i + 2).is_some_and(|c| c.eq_ignore_ascii_case(&b'm'))
                            && b.get(i + 3) == Some(&b'!')),
                    "Executable comments and optimizer hints are not allowed"
                );
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                ensure!(i + 1 < b.len(), "Unterminated SQL comment");
                i += 2;
            }
            _ => i += 1,
        }
    }
    Ok(())
}
// Run before parsing: AST visitors cannot protect the parser, or the recursive
// drop of an AST rejected by a visitor. The byte cap still permits 20,000 `+`
// operators. Tokenization is flat and bounded by SQL_CAP. These deliberately
// conservative budgets also cover unary chains, set operations and CASE (which
// can nest without parentheses), while allowing wide projections and IN lists.
fn check_complexity(tokens: &[Token]) -> Result<()> {
    let mut count = 0;
    let mut nesting = 0usize;
    let mut cases = 0usize;
    let mut operators = 0;
    for token in tokens {
        if matches!(token, Token::Whitespace(_)) {
            continue;
        }
        count += 1;
        ensure!(
            count <= TOKEN_CAP,
            "SQL query exceeds the token complexity limit"
        );
        match token {
            Token::LParen | Token::LBracket | Token::LBrace => nesting += 1,
            Token::RParen | Token::RBracket | Token::RBrace => {
                nesting = nesting.saturating_sub(1);
            }
            Token::Word(word) => {
                if word.keyword == Keyword::CASE {
                    cases += 1;
                } else if word.keyword == Keyword::END {
                    cases = cases.saturating_sub(1);
                }
                if matches!(
                    word.keyword,
                    Keyword::AND
                        | Keyword::OR
                        | Keyword::XOR
                        | Keyword::NOT
                        | Keyword::IS
                        | Keyword::BETWEEN
                        | Keyword::IN
                        | Keyword::LIKE
                        | Keyword::ILIKE
                        | Keyword::SIMILAR
                        | Keyword::REGEXP
                        | Keyword::RLIKE
                        | Keyword::MATCH
                        | Keyword::MEMBER
                        | Keyword::NOTNULL
                        | Keyword::OVERLAPS
                        | Keyword::OPERATOR
                        | Keyword::DIV
                        | Keyword::MOD
                        | Keyword::COLLATE
                        | Keyword::AT
                        | Keyword::CASE
                        | Keyword::WHEN
                        | Keyword::UNION
                        | Keyword::EXCEPT
                        | Keyword::INTERSECT
                        | Keyword::JOIN
                ) {
                    operators += 1;
                }
            }
            Token::Number(..)
            | Token::Comma
            | Token::Period
            | Token::SemiColon
            | Token::SingleQuotedString(_)
            | Token::DoubleQuotedString(_)
            | Token::NationalStringLiteral(_)
            | Token::HexStringLiteral(_)
            | Token::Placeholder(_) => (),
            // Count other punctuation/operators conservatively, including
            // dialect extensions, rather than missing a recursive operator.
            _ => operators += 1,
        }
        ensure!(
            nesting + cases <= NESTING_CAP,
            "SQL query exceeds the nesting complexity limit"
        );
        ensure!(
            operators <= OPERATOR_CAP,
            "SQL query exceeds the operator complexity limit"
        );
    }
    Ok(())
}
fn select_body(body: &SetExpr) -> bool {
    // Bound union-tree traversal before the derived recursive visitor runs.
    // The SQL byte cap alone permits thousands of left-deep UNION nodes.
    let mut pending = vec![body];
    let mut count = 0;
    while let Some(body) = pending.pop() {
        count += 1;
        if count > 256 {
            return false;
        }
        match body {
            SetExpr::Select(_) | SetExpr::Query(_) => (),
            SetExpr::SetOperation { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            _ => return false,
        }
    }
    true
}
struct ReadOnly;
impl Visitor for ReadOnly {
    type Break = &'static str;
    fn pre_visit_statement(&mut self, s: &Statement) -> ControlFlow<Self::Break> {
        if !matches!(s, Statement::Query(_)) {
            return ControlFlow::Break("Only a SELECT query is allowed");
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_query(&mut self, q: &Query) -> ControlFlow<Self::Break> {
        if !select_body(&q.body)
            || !q.locks.is_empty()
            || q.for_clause.is_some()
            || q.settings.is_some()
            || q.format_clause.is_some()
            || !q.pipe_operators.is_empty()
        {
            return ControlFlow::Break("Only non-locking SELECT queries are allowed");
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_select(&mut self, s: &Select) -> ControlFlow<Self::Break> {
        if s.into.is_some()
            || !s.optimizer_hints.is_empty()
            || s.select_modifiers.as_ref().is_some_and(|m| m.is_any_set())
        {
            return ControlFlow::Break("SELECT INTO, hints, and SELECT modifiers are not allowed");
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_table_factor(&mut self, t: &TableFactor) -> ControlFlow<Self::Break> {
        match t {
            TableFactor::Table {
                args: None,
                with_hints,
                version: None,
                ..
            } if with_hints.is_empty() => (),
            TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => (),
            _ => {
                return ControlFlow::Break(
                    "Table functions and special table sources are not allowed",
                );
            }
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_expr(&mut self, e: &Expr) -> ControlFlow<Self::Break> {
        if matches!(
            e,
            Expr::BinaryOp {
                op: BinaryOperator::Assignment,
                ..
            }
        ) {
            return ControlFlow::Break("Variable assignment is not allowed");
        }
        if let Expr::Function(f) = e {
            let name = if f.name.0.len() == 1 {
                f.name.0[0]
                    .as_ident()
                    .filter(|i| i.quote_style.is_none())
                    .map(|i| i.value.to_ascii_uppercase())
            } else {
                None
            };
            if !name.as_deref().is_some_and(|n| {
                matches!(
                    n,
                    "COUNT"
                        | "SUM"
                        | "MIN"
                        | "MAX"
                        | "AVG"
                        | "COALESCE"
                        | "IFNULL"
                        | "NULLIF"
                        | "IF"
                        | "LENGTH"
                        | "CHAR_LENGTH"
                        | "CHARACTER_LENGTH"
                        | "LOWER"
                        | "UPPER"
                        | "TRIM"
                        | "LTRIM"
                        | "RTRIM"
                        | "CONCAT"
                        | "CONCAT_WS"
                        | "SUBSTRING"
                        | "SUBSTR"
                        | "LEFT"
                        | "RIGHT"
                        | "REPLACE"
                        | "ROUND"
                        | "ABS"
                        | "CEIL"
                        | "CEILING"
                        | "FLOOR"
                        | "MOD"
                        | "POWER"
                        | "SQRT"
                        | "DATE"
                        | "YEAR"
                        | "MONTH"
                        | "DAY"
                        | "DAYOFMONTH"
                        | "DATE_FORMAT"
                        | "DATEDIFF"
                        | "NOW"
                        | "CURRENT_TIMESTAMP"
                        | "CURRENT_DATE"
                        | "CURRENT_TIME"
                        | "UTC_TIMESTAMP"
                        | "ROW_NUMBER"
                        | "RANK"
                        | "DENSE_RANK"
                        | "LAG"
                        | "LEAD"
                        | "FIRST_VALUE"
                        | "LAST_VALUE"
                        | "JSON_EXTRACT"
                        | "JSON_UNQUOTE"
                        | "JSON_LENGTH"
                        | "JSON_VALID"
                )
            }) {
                return ControlFlow::Break(
                    "Only supported unqualified built-in functions are allowed",
                );
            }
        }
        ControlFlow::Continue(())
    }
}
/// Validates without connecting. Errors never contain SQL, parser diagnostics,
/// identifiers, literals, or the original error as a source.
pub fn validate_read_only(sql: &str) -> Result<()> {
    ensure!(
        !sql.is_empty() && sql.len() <= SQL_CAP,
        "SQL must contain 1 through 65536 bytes"
    );
    check_comments(sql)?;
    let dialect = MySqlDialect {};
    let tokens = Tokenizer::new(&dialect, sql)
        .tokenize()
        .map_err(|_| anyhow!("SQL could not be parsed; check query syntax"))?;
    check_complexity(&tokens)?;
    let meaningful: Vec<_> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace(_)))
        .collect();
    let delimiters: Vec<_> = meaningful
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, Token::SemiColon))
        .map(|(i, _)| i)
        .collect();
    ensure!(
        delimiters.is_empty() || delimiters == [meaningful.len() - 1],
        "Provide exactly one query with at most one trailing delimiter"
    );
    let statements = Parser::parse_sql(&dialect, sql)
        .map_err(|_| anyhow!("SQL could not be parsed; check query syntax"))?;
    ensure!(statements.len() == 1, "Provide exactly one SELECT query");
    if let ControlFlow::Break(reason) = statements[0].visit(&mut ReadOnly) {
        return Err(anyhow!(reason));
    }
    Ok(())
}

/// Native prepared statement execution, capped without adding LIMIT or paging
/// clauses to user SQL. `has_more` means omitted rows; `next_offset` is always
/// None, because arbitrary query results cannot safely be paged by rewriting.
pub async fn execute_read_only(
    profile: &SourceProfile,
    password: &str,
    request: &QueryRequest,
) -> Result<QueryResult> {
    ensure!(
        (1..=200).contains(&request.limit),
        "Query limit must be 1 through 200"
    );
    validate_read_only(&request.sql)?;
    let start = Instant::now();
    mysql::bounded(async {
        let mut session = Session::connect(profile, password).await?;
        // Pin lexer semantics: no ANSI_QUOTES or NO_BACKSLASH_ESCAPES ambiguity.
        session
            .conn()
            .query_drop("SET SESSION sql_mode = 'STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION'")
            .await
            .map_err(mysql::driver_error)?;
        let deadline = match profile.engine {
            DbEngine::MySql => "SET SESSION MAX_EXECUTION_TIME = 20000",
            DbEngine::MariaDb => "SET SESSION max_statement_time = 20",
        };
        session
            .conn()
            .query_drop(deadline)
            .await
            .map_err(mysql::driver_error)?;
        session
            .conn()
            .query_drop("START TRANSACTION READ ONLY")
            .await
            .map_err(mysql::driver_error)?;
        let mut result = session
            .conn()
            .exec_iter(&request.sql, ())
            .await
            .map_err(mysql::driver_error)?;
        let native = result.columns_ref();
        ensure!(
            !native.is_empty() && native.len() <= mysql::COLUMN_CAP,
            "Query must return 1 through 512 columns"
        );
        let columns: Vec<_> = native
            .iter()
            .map(|c| ColumnInfo {
                name: c.name_str().into_owned(),
                data_type: data_type(c.column_type()),
                nullable: !c.flags().contains(ColumnFlags::NOT_NULL_FLAG),
                is_primary_key: c.flags().contains(ColumnFlags::PRI_KEY_FLAG),
            })
            .collect();
        let binary: Vec<_> = native.iter().map(|c| c.character_set() == 63).collect();
        let mut preview = Preview::default();
        while let Some(row) = result.next().await.map_err(mysql::driver_error)? {
            if preview.rows.len() >= request.limit as usize {
                preview.has_more = true;
                break;
            }
            let values =
                mysql::decode_row(row.unwrap_raw(), &columns, &binary, &mut preview.truncated)?;
            if !preview.push(values)? {
                break;
            }
        }
        drop(result);
        session.close_transport();
        let mut warnings = Vec::new();
        if preview.has_more {
            warnings.push(
                "Result limited by the row or preview-byte cap; no query pagination is available"
                    .into(),
            );
        }
        if preview.truncated {
            warnings.push("Some cell values or rows were truncated by the preview caps".into());
        }
        Ok(QueryResult {
            page: TablePage {
                columns,
                rows: preview.rows,
                has_more: preview.has_more,
                next_offset: None,
                offset: 0,
                truncated: preview.truncated,
            },
            elapsed_ms: start.elapsed().as_millis().min(u64::MAX as u128) as u64,
            warnings,
        })
    })
    .await
}
fn data_type(t: ColumnType) -> String {
    // Native metadata, not an inferred type from the first row. In particular
    // decimal and JSON must retain the existing decoder's text semantics.
    match t {
        ColumnType::MYSQL_TYPE_DECIMAL | ColumnType::MYSQL_TYPE_NEWDECIMAL => "decimal".into(),
        ColumnType::MYSQL_TYPE_JSON => "json".into(),
        ColumnType::MYSQL_TYPE_DATE | ColumnType::MYSQL_TYPE_NEWDATE => "date".into(),
        other => format!("{other:?}")
            .trim_start_matches("MYSQL_TYPE_")
            .to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn select_cte_union_and_literals() {
        for sql in [
            "SELECT 1",
            "SELECT 'a;b' AS text;",
            "SELECT '/*!50000 DELETE */', '/*M!', '#';",
            "SELECT `a;b` FROM `t`",
            "SELECT COUNT(DISTINCT id), SUM(price) FROM contact",
            "WITH x AS (SELECT 1 AS id) SELECT id FROM x UNION ALL SELECT 2",
            "SELECT (SELECT MAX(id) FROM contact)",
            "# heading\n SELECT 1; -- tail\n",
            "SELECT 'a\\\'b;';",
            "SELECT CAST(1 AS DECIMAL(10,2)), COALESCE(NULL, 2)",
            "SELECT LOWER(name) FROM contact ORDER BY id LIMIT 3",
            "WITH totals AS (SELECT account_id, SUM(price) AS total FROM orders GROUP BY account_id) SELECT account_id, total FROM totals WHERE total > 10 UNION ALL SELECT id, 0 FROM accounts WHERE id NOT IN (SELECT account_id FROM totals)",
        ] {
            validate_read_only(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
        }
    }
    #[test]
    fn denies_multiple_and_empty_statements() {
        for sql in [
            "",
            " ",
            "-- empty",
            ";",
            "SELECT 1;;",
            "SELECT 1; SELECT 2",
            "SELECT 1; DROP TABLE t",
        ] {
            assert!(validate_read_only(sql).is_err(), "{sql}");
        }
    }
    #[test]
    fn denies_writes_and_session_statements() {
        for sql in [
            "INSERT INTO t VALUES (1)",
            "UPDATE t SET x=1",
            "DELETE FROM t",
            "DROP TABLE t",
            "SET @x=1",
            "CALL p()",
            "EXPLAIN SELECT 1",
            "EXPLAIN ANALYZE SELECT 1",
            "SHOW TABLES",
            "WITH x AS (DELETE FROM t RETURNING id) SELECT * FROM x",
        ] {
            assert!(validate_read_only(sql).is_err(), "{sql}");
        }
    }
    #[test]
    fn denies_into_everywhere() {
        for sql in [
            "SELECT 1 INTO OUTFILE '/tmp/x'",
            "SELECT 1 INTO DUMPFILE '/tmp/x'",
            "SELECT 1 INTO @x",
            "SELECT 1 INTO t",
            "WITH x AS (SELECT 1 INTO t) SELECT * FROM x",
            "SELECT 1 UNION SELECT 2 INTO t",
        ] {
            assert!(validate_read_only(sql).is_err(), "{sql}");
        }
    }
    #[test]
    fn denies_locks_in_nested_queries() {
        for sql in [
            "SELECT * FROM t FOR UPDATE",
            "SELECT * FROM t FOR SHARE",
            "SELECT * FROM t LOCK IN SHARE MODE",
            "WITH x AS (SELECT * FROM t FOR UPDATE) SELECT * FROM x",
        ] {
            assert!(validate_read_only(sql).is_err(), "{sql}");
        }
    }
    #[test]
    fn denies_functions_and_assignments() {
        for sql in [
            "SELECT SLEEP(1)",
            "SELECT BENCHMARK(1, 2)",
            "SELECT GET_LOCK('x', 1)",
            "SELECT RELEASE_LOCK('x')",
            "SELECT IS_FREE_LOCK('x')",
            "SELECT LOAD_FILE('/tmp/x')",
            "SELECT custom_function()",
            "SELECT db.COUNT(1)",
            "SELECT `COUNT`(1)",
            "SELECT @x := 1",
            "SELECT 1 FROM custom_function(1)",
            "SELECT (SELECT SLEEP(1))",
        ] {
            assert!(validate_read_only(sql).is_err(), "{sql}");
        }
    }
    #[test]
    fn function_names_as_columns_are_allowed() {
        validate_read_only("SELECT sleep, get_lock FROM t").unwrap();
    }
    #[test]
    fn rejects_active_version_comments_and_hints() {
        for sql in [
            "SELECT 1 /*!50000 INTO OUTFILE 'x' */",
            "SELECT 1 /*M!100000 INTO OUTFILE 'x' */",
            "SELECT /*+ MAX_EXECUTION_TIME(0) */ 1",
            "SELECT 1--not_a_mysql_comment /*!50000 hi */",
            "SELECT 1--SLEEP(1)",
        ] {
            assert!(validate_read_only(sql).is_err(), "{sql}");
        }
    }
    #[test]
    fn ordinary_comments_can_contain_markers() {
        validate_read_only("/* outer /*! */ SELECT 1 -- /*!\n # /*M!\n").unwrap();
    }
    #[test]
    fn errors_are_sanitized_and_size_is_capped() {
        for sql in [
            "SELECT sentinel_secret ???".to_owned(),
            format!("SELECT '{}'", "sentinel_secret".repeat(6000)),
        ] {
            let error = validate_read_only(&sql).unwrap_err();
            assert!(!format!("{error:#?}").contains("sentinel_secret"));
            assert_eq!(error.chain().count(), 1);
        }
    }
    #[test]
    fn native_types_preserve_decimal_and_json() {
        assert_eq!(data_type(ColumnType::MYSQL_TYPE_NEWDECIMAL), "decimal");
        assert_eq!(data_type(ColumnType::MYSQL_TYPE_JSON), "json");
    }
    #[test]
    fn pathological_union_tree_is_rejected() {
        let sql = std::iter::repeat_n("SELECT 1", 1000)
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        assert!(validate_read_only(&sql).is_err());
    }
    #[test]
    fn rejects_expression_chains_before_parsing() {
        for sql in [
            format!("SELECT 1{}", "+1".repeat(20_000)),
            format!("SELECT 1{}", "+1".repeat(OPERATOR_CAP + 1)),
            format!("SELECT {}1", "NOT ".repeat(OPERATOR_CAP + 1)),
            format!("SELECT 1{}", " OR 1".repeat(OPERATOR_CAP + 1)),
            format!("SELECT 1{}", " IS NULL".repeat(OPERATOR_CAP + 1)),
            format!("SELECT 'x'{}", " SIMILAR TO 'x'".repeat(OPERATOR_CAP + 1)),
            format!("SELECT 1{}", " -> 'x'".repeat(OPERATOR_CAP + 1)),
        ] {
            assert!(sql.len() < SQL_CAP);
            let error = validate_read_only(&sql).unwrap_err();
            assert!(error.to_string().contains("complexity limit"));
            assert_eq!(error.chain().count(), 1);
        }
        validate_read_only(&format!("SELECT 1{}", "+1".repeat(OPERATOR_CAP))).unwrap();
    }
    #[test]
    fn bounds_nesting_including_cases_without_parentheses() {
        for depth in [NESTING_CAP, NESTING_CAP + 1] {
            for sql in [
                format!("SELECT {}1{}", "(".repeat(depth), ")".repeat(depth)),
                format!(
                    "SELECT {}1{}",
                    "CASE WHEN TRUE THEN ".repeat(depth),
                    " ELSE 0 END".repeat(depth)
                ),
            ] {
                if depth == NESTING_CAP {
                    validate_read_only(&sql).unwrap();
                } else {
                    assert_eq!(
                        validate_read_only(&sql).unwrap_err().to_string(),
                        "SQL query exceeds the nesting complexity limit"
                    );
                }
            }
        }
    }
    #[test]
    fn flat_token_budget_and_wide_queries() {
        // SELECT plus 2048 values and 2047 commas is exactly 4096 tokens.
        let at_limit = format!("SELECT {}", vec!["1"; TOKEN_CAP / 2].join(","));
        validate_read_only(&at_limit).unwrap();
        assert_eq!(
            validate_read_only(&format!("{at_limit},1"))
                .unwrap_err()
                .to_string(),
            "SQL query exceeds the token complexity limit"
        );
        let columns = (0..512).map(|i| format!("t.c{i}")).collect::<Vec<_>>();
        validate_read_only(&format!("SELECT {} FROM db.t", columns.join(","))).unwrap();
        validate_read_only(&format!(
            "SELECT id FROM t WHERE id IN ({})",
            vec!["1"; 1000].join(",")
        ))
        .unwrap();
    }
    #[test]
    fn complexity_markers_in_literals_identifiers_and_comments_are_ignored() {
        let markers = "( CASE NOT + OR [".repeat(300);
        validate_read_only(&format!(
            "SELECT '{markers}', `{markers}` FROM t /* {markers} */ -- {markers}\n # {markers}\n"
        ))
        .unwrap();
    }
}
