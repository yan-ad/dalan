//! Source-preserving statement boundaries, not query validation or authorization.
use anyhow::{Result, anyhow, ensure};
use dalan_drivers::DbEngine;
use sqlparser::{
    dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect},
    tokenizer::{Token, Tokenizer, Whitespace},
};
use std::{collections::HashMap, ops::Range};
use unicode_segmentation::UnicodeSegmentation;

const MAX_SQL_BYTES: usize = 64 * 1024;
const MAX_STATEMENTS: usize = 64;
const MAX_LABEL_GRAPHEMES: usize = 80;
const MAX_LABEL_BYTES: usize = 320;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlStatement {
    /// UTF-8 byte offsets in the original input, excluding the delimiter.
    pub range: Range<usize>,
    /// Zero-based line of the first meaningful token (not its leading comments).
    pub start_line: usize,
    pub label: String,
}

// sqlparser expands /*! ... */ into synthetic tokens with synthetic locations.
// Keep the base dialect's lexical behavior and identity, but retain comments as
// single source tokens. Executable comments must reach the separate validator,
// including when they are the only content of a statement.
#[derive(Debug)]
struct SourceDialect(&'static dyn Dialect);
macro_rules! lexical_flags {
    ($($method:ident),* $(,)?) => {$(
        fn $method(&self) -> bool { self.0.$method() }
    )*};
}
impl Dialect for SourceDialect {
    fn dialect(&self) -> std::any::TypeId {
        self.0.dialect()
    }
    fn is_identifier_start(&self, ch: char) -> bool {
        self.0.is_identifier_start(ch)
    }
    fn is_identifier_part(&self, ch: char) -> bool {
        self.0.is_identifier_part(ch)
    }
    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        self.0.is_delimited_identifier_start(ch)
    }
    fn is_custom_operator_part(&self, ch: char) -> bool {
        self.0.is_custom_operator_part(ch)
    }
    fn supports_multiline_comment_hints(&self) -> bool {
        false
    }
    lexical_flags!(
        ignores_wildcard_escapes,
        requires_single_line_comment_whitespace,
        supports_dollar_as_money_prefix,
        supports_dollar_placeholder,
        supports_geometric_types,
        supports_nested_comments,
        supports_numeric_literal_underscores,
        supports_numeric_prefix,
        supports_pipe_operator,
        supports_quote_delimited_string,
        supports_string_escape_constant,
        supports_string_literal_backslash_escape,
        supports_triple_quoted_string,
        supports_unicode_string_literal,
    );
}

/// Split SQL on tokenizer semicolons without rewriting its bytes. Empty and
/// ordinary comment-only spans are omitted. Leading comments stay attached to
/// the next statement; comments before its delimiter stay in its range. Trailing
/// comment-only spans after a delimiter are omitted. Executable comments/hints
/// count as content, so they cannot bypass downstream validation. JDBC uses the
/// generic dialect; MongoDB/Redis inputs remain one opaque, trimmed command.
///
/// Lexical errors are intentionally sanitized: no SQL or tokenizer error text
/// is returned. Successful splitting says nothing about whether execution is safe.
pub fn split_statements(sql: &str, engine: DbEngine) -> Result<Vec<SqlStatement>> {
    ensure!(sql.len() <= MAX_SQL_BYTES, "SQL exceeds the 64 KiB limit");
    ensure!(
        !sql.chars()
            .any(|c| c.is_control() && !matches!(c, '\t' | '\r' | '\n')),
        "Input contains unsupported control characters"
    );
    if matches!(engine, DbEngine::MongoDb | DbEngine::Redis) {
        let mut result = Vec::new();
        if !sql.trim().is_empty() {
            let start = sql.len() - sql.trim_start().len();
            push_statement(
                sql,
                start..sql.len(),
                start,
                sql[..start].bytes().filter(|b| *b == b'\n').count(),
                &mut result,
            )?;
        }
        return Ok(result);
    }
    let base: &'static dyn Dialect = match engine {
        DbEngine::MySql | DbEngine::MariaDb => &MySqlDialect {},
        DbEngine::PostgreSql => &PostgreSqlDialect {},
        _ => &GenericDialect {},
    };
    let dialect = SourceDialect(base);
    let tokens = Tokenizer::new(&dialect, sql)
        .with_unescape(false)
        .tokenize_with_location()
        .map_err(|_| anyhow!("Cannot determine SQL statement boundaries: malformed SQL"))?;

    // Tokenizer columns count characters, not bytes; map locations once rather
    // than repeatedly scanning prefixes (also handles UTF-8 and CRLF exactly).
    let mut positions = HashMap::new();
    let (mut line, mut column) = (1u64, 1u64);
    for (offset, ch) in sql.char_indices() {
        positions.insert((line, column), offset);
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    positions.insert((line, column), sql.len());
    let mut result = Vec::new();
    let mut segment_start = 0;
    let mut meaningful = None;
    let mut meaningful_is_comment = false;
    let mut previous_end = 0;
    for token in tokens {
        let offset = |loc: sqlparser::tokenizer::Location| {
            positions
                .get(&(loc.line, loc.column))
                .copied()
                .ok_or_else(|| anyhow!("Cannot determine SQL source boundaries"))
        };
        let start = offset(token.span.start)?;
        let end = offset(token.span.end)?;
        ensure!(
            start == previous_end && end > start,
            "Cannot determine SQL source boundaries"
        );
        let raw = sql
            .get(start..end)
            .ok_or_else(|| anyhow!("Cannot determine SQL source boundaries"))?;
        previous_end = end;
        if matches!(token.token, Token::SemiColon) {
            if let Some((first, first_line)) = meaningful.take() {
                push_statement(sql, segment_start..start, first, first_line, &mut result)?;
            }
            segment_start = end;
            meaningful_is_comment = false;
        } else {
            let executable = matches!(
                &token.token,
                Token::Whitespace(Whitespace::MultiLineComment(_))
            ) && (raw.starts_with("/*!")
                || raw.starts_with("/*+")
                || raw.get(..4).is_some_and(|s| s.eq_ignore_ascii_case("/*M!")));
            let actual_token = !matches!(token.token, Token::Whitespace(_))
                && !raw.chars().all(char::is_whitespace);
            if (meaningful.is_none() && executable)
                || (actual_token && (meaningful.is_none() || meaningful_is_comment))
            {
                meaningful = Some((start, (token.span.start.line - 1) as usize));
                meaningful_is_comment = !actual_token;
            }
        }
    }
    ensure!(
        previous_end == sql.len(),
        "Cannot determine SQL source boundaries"
    );
    if let Some((first, first_line)) = meaningful {
        push_statement(
            sql,
            segment_start..sql.len(),
            first,
            first_line,
            &mut result,
        )?;
    }
    Ok(result)
}

fn push_statement(
    sql: &str,
    range: Range<usize>,
    first: usize,
    start_line: usize,
    result: &mut Vec<SqlStatement>,
) -> Result<()> {
    ensure!(
        result.len() < MAX_STATEMENTS,
        "SQL exceeds the 64 statement limit"
    );
    let raw = &sql[range.clone()];
    let start = range.start + raw.len() - raw.trim_start().len();
    let end = range.start + raw.trim_end().len();
    result.push(SqlStatement {
        range: start..end,
        start_line,
        label: statement_label(&sql[first..end], result.len() + 1),
    });
    Ok(())
}

fn statement_label(sql: &str, ordinal: usize) -> String {
    let compact = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let suffix = format!(" (No. {ordinal})");
    if compact.graphemes(true).count() + suffix.graphemes(true).count() <= MAX_LABEL_GRAPHEMES
        && compact.len() + suffix.len() <= MAX_LABEL_BYTES
    {
        return compact + &suffix;
    }
    let mut label = String::new();
    let grapheme_budget = MAX_LABEL_GRAPHEMES - suffix.graphemes(true).count() - 1;
    let byte_budget = MAX_LABEL_BYTES - suffix.len() - '…'.len_utf8();
    for grapheme in compact.graphemes(true).take(grapheme_budget) {
        if label.len() + grapheme.len() > byte_budget {
            break;
        }
        label.push_str(grapheme);
    }
    label.push('…');
    label.push_str(&suffix);
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slices<'a>(sql: &'a str, statements: &[SqlStatement]) -> Vec<&'a str> {
        statements.iter().map(|s| &sql[s.range.clone()]).collect()
    }
    #[test]
    fn exact_bytes_comments_lines_and_ordinals() {
        let sql = "\n  -- leading; café\n /* ordinary */\n SELECT '東京;';\n\n /* next; */ SELECT 2 /* retained; */; -- trailing;\n";
        let statements = split_statements(sql, DbEngine::MySql).unwrap();
        assert_eq!(
            slices(sql, &statements),
            vec![
                "-- leading; café\n /* ordinary */\n SELECT '東京;'",
                "/* next; */ SELECT 2 /* retained; */"
            ]
        );
        assert_eq!(statements[0].start_line, 3);
        assert_eq!(statements[1].start_line, 5);
        assert_eq!(statements[0].label, "SELECT '東京;' (No. 1)");
        assert_eq!(statements[1].label, "SELECT 2 /* retained; */ (No. 2)");
    }
    #[test]
    fn mysql_quotes_escapes_and_comment_rules() {
        let sql = "SELECT 'a\\';b', \"c;d\", `a;b`, 'it''s;ok', 'multi\nline;'; # note;\n SELECT 1--2;SELECT 3";
        for engine in [DbEngine::MySql, DbEngine::MariaDb] {
            let statements = split_statements(sql, engine).unwrap();
            assert_eq!(statements.len(), 3);
            assert!(slices(sql, &statements)[0].ends_with("'multi\nline;'"));
            assert_eq!(slices(sql, &statements)[1], "# note;\n SELECT 1--2");
            assert_eq!(statements[1].start_line, 2);
        }
    }
    #[test]
    fn postgres_and_generic_dollar_quoting_and_nested_comments() {
        let sql = "SELECT $$one;\ntwo$$, $tag$x;y$tag$; /* outer /* ; */ inner */ SELECT 'x'';y';";
        for engine in [DbEngine::PostgreSql, DbEngine::Jdbc] {
            let statements = split_statements(sql, engine).unwrap();
            assert_eq!(statements.len(), 2);
            assert_eq!(
                slices(sql, &statements)[0],
                "SELECT $$one;\ntwo$$, $tag$x;y$tag$"
            );
            assert_eq!(statements[1].start_line, 1);
        }
        assert_eq!(
            split_statements("SELECT E'a\\';b'; SELECT 2", DbEngine::PostgreSql)
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn executable_comments_cannot_disappear_or_create_synthetic_spans() {
        for engine in [DbEngine::MySql, DbEngine::MariaDb, DbEngine::Jdbc] {
            let sql = "/*!50100 SELECT 1; SELECT 2 */; /*M! SELECT 3; */; /*+ hint */ SELECT 4;";
            let statements = split_statements(sql, engine).unwrap();
            assert_eq!(
                slices(sql, &statements),
                vec![
                    "/*!50100 SELECT 1; SELECT 2 */",
                    "/*M! SELECT 3; */",
                    "/*+ hint */ SELECT 4"
                ]
            );
            assert_eq!(split_statements("/*!*/", engine).unwrap().len(), 1);
            let leading = "/*+ hint */\n SELECT 1";
            let statement = split_statements(leading, engine).unwrap().remove(0);
            assert_eq!(&leading[statement.range], leading);
            assert_eq!(statement.start_line, 1);
            assert_eq!(statement.label, "SELECT 1 (No. 1)");
        }
    }
    #[test]
    fn empty_and_comment_only_spans_do_not_count() {
        for sql in [
            "",
            " \n\t",
            ";;;",
            "-- x;\n /* ; */; # end",
            &";".repeat(128),
        ] {
            assert!(split_statements(sql, DbEngine::MySql).unwrap().is_empty());
        }
        assert_eq!(
            split_statements(";;;SELECT 1;; /* ignored */; SELECT 2", DbEngine::MySql).unwrap()[1]
                .label,
            "SELECT 2 (No. 2)"
        );
    }
    #[test]
    fn non_sql_commands_are_opaque() {
        let sql = " \n {\"command\":\"a;b\",\"other\":\"'/*\"} \n ";
        for engine in [DbEngine::MongoDb, DbEngine::Redis] {
            let statements = split_statements(sql, engine).unwrap();
            assert_eq!(slices(sql, &statements), vec![sql.trim()]);
            assert_eq!(statements[0].start_line, 1);
            assert!(split_statements("  \n", engine).unwrap().is_empty());
        }
    }
    #[test]
    fn malformed_and_controls_return_sanitized_errors() {
        for sql in [
            "SELECT 'private",
            "SELECT `private",
            "SELECT /* private",
            "SELECT \0private",
            "SELECT \u{1b}private",
        ] {
            let error = split_statements(sql, DbEngine::MySql).unwrap_err();
            assert!(!format!("{error:#}").contains("private"));
        }
        assert!(split_statements("SELECT $secret$private", DbEngine::PostgreSql).is_err());
    }
    #[test]
    fn size_and_statement_budgets() {
        assert!(split_statements(&"x".repeat(MAX_SQL_BYTES), DbEngine::MySql).is_ok());
        for engine in [DbEngine::MySql, DbEngine::MongoDb] {
            assert!(split_statements(&"x".repeat(MAX_SQL_BYTES + 1), engine).is_err());
        }
        assert_eq!(
            split_statements(&"SELECT 1;".repeat(64), DbEngine::MySql)
                .unwrap()
                .len(),
            64
        );
        assert!(split_statements(&"SELECT 1;".repeat(65), DbEngine::MySql).is_err());
        assert!(split_statements(&"SELECT 1;".repeat(128), DbEngine::MySql).is_err());
    }
    #[test]
    fn label_limits_graphemes_bytes_unicode_whitespace_and_controls() {
        for text in [
            "é".repeat(200),
            "👩‍👩‍👧‍👧".repeat(50),
            "a\u{301}".repeat(200),
            format!("SELECT\n\t1\u{a0} {}", "界".repeat(200)),
            format!("a{}", "\u{301}".repeat(400)),
        ] {
            let label = statement_label(&text, 64);
            assert!(label.len() <= MAX_LABEL_BYTES);
            assert!(label.graphemes(true).count() <= MAX_LABEL_GRAPHEMES);
            assert!(!label.chars().any(char::is_control));
            assert!(label.ends_with("… (No. 64)"));
        }
        assert_eq!(
            statement_label("SELECT\n\t1\u{a0}  2", 1),
            "SELECT 1 2 (No. 1)"
        );
    }
    #[test]
    fn crlf_and_multibyte_locations_are_exact() {
        let sql = "\r\n /* café */ SELECT '界\r\n;';\r\n SELECT 2;";
        let statements = split_statements(sql, DbEngine::MySql).unwrap();
        assert_eq!(
            slices(sql, &statements),
            vec!["/* café */ SELECT '界\r\n;'", "SELECT 2"]
        );
        assert_eq!(statements[0].start_line, 1);
        assert_eq!(statements[1].start_line, 3);
    }
}
