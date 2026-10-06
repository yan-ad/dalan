//! Small, conservative local SQL tools; no parser round-trip or database execution.
//! Token spans preserve spelling of literals, identifiers and comments. Unsupported
//! lexing returns the original input. This is not a full dialect-aware SQL formatter.
//! File publication is atomic/no-clobber; portable std checks cannot defend against
//! a hostile concurrent replacement of an ancestor directory.
use anyhow::{Result, bail, ensure};
use sqlparser::{
    dialect::MySqlDialect,
    keywords::Keyword,
    tokenizer::{Location, Token, Tokenizer, Whitespace},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

pub const MAX_SQL_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlTransform {
    Format { uppercase: bool },
    Compress,
}
impl SqlTransform {
    pub fn transform(self, sql: &str) -> String {
        match self {
            Self::Format { uppercase } => format_sql(sql, uppercase),
            Self::Compress => compress_sql(sql),
        }
    }
}

struct Piece<'a> {
    token: Token,
    raw: &'a str,
}

fn pieces(sql: &str) -> Option<Vec<Piece<'_>>> {
    // Executable comments/hints must never be re-tokenized or moved. Returning
    // unchanged also handles MariaDB executable comments and unsupported controls.
    if sql.len() > MAX_SQL_BYTES
        || sql.contains("/*!")
        || sql.contains("/*M!")
        || sql.contains("/*+")
        // Dollar quoting / positional placeholders belong to other dialects.
        || sql.contains('$')
        || sql
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\t' | '\r' | '\n'))
    {
        return None;
    }
    let tokens = Tokenizer::new(&MySqlDialect {}, sql)
        .with_unescape(false)
        .tokenize_with_location()
        .ok()?;
    // Locations count Unicode characters, not bytes. Build a linear map once.
    let mut positions = std::collections::HashMap::new();
    let (mut line, mut column) = (1, 1);
    for (offset, c) in sql.char_indices() {
        positions.insert((line, column), offset);
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    positions.insert((line, column), sql.len());
    let offset = |loc: Location| positions.get(&(loc.line, loc.column)).copied();
    let mut end = 0;
    let mut result = Vec::new();
    for t in tokens {
        let start = offset(t.span.start)?;
        let next = offset(t.span.end)?;
        // Unknown characters or overlapping hint spans are unsafe to rewrite.
        if start != end
            || next <= start
            || matches!(t.token, Token::Char(_) | Token::LBracket | Token::RBracket)
        {
            return None;
        }
        result.push(Piece {
            token: t.token,
            raw: sql.get(start..next)?,
        });
        end = next;
    }
    (end == sql.len()).then_some(result)
}

fn gap(token: &Token) -> bool {
    matches!(
        token,
        Token::Whitespace(Whitespace::Space | Whitespace::Tab | Whitespace::Newline)
    )
}

fn boundary(token: &Token) -> bool {
    matches!(token, Token::Word(w) if w.quote_style.is_none() && matches!(w.keyword,
        Keyword::SELECT | Keyword::FROM | Keyword::WHERE | Keyword::GROUP |
        Keyword::ORDER | Keyword::HAVING | Keyword::LIMIT | Keyword::JOIN |
        Keyword::UNION | Keyword::LEFT | Keyword::RIGHT | Keyword::INNER |
        Keyword::OUTER | Keyword::CROSS))
}

/// Format existing token gaps and keyword case, never arbitrary identifier case.
/// No whitespace is inserted into an originally adjacent token pair.
pub fn format_sql(sql: &str, uppercase: bool) -> String {
    transform(sql, Some(uppercase))
}

/// Collapse runs of whitespace without merging adjacent tokens. Newline gaps
/// retain a newline (including implicit adjacent-string concatenation semantics).
pub fn compress_sql(sql: &str) -> String {
    transform(sql, None)
}

fn transform(sql: &str, case: Option<bool>) -> String {
    let Some(pieces) = pieces(sql) else {
        return sql.to_owned();
    };
    let mut out = String::with_capacity(sql.len());
    let mut pending = String::new();
    let mut depth = 0usize;
    for piece in pieces {
        if gap(&piece.token) {
            pending.push_str(piece.raw);
            continue;
        }
        if !out.is_empty() && !pending.is_empty() {
            if case.is_some() && boundary(&piece.token) {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&"  ".repeat(depth.min(32)));
            } else if pending.contains(['\n', '\r']) {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            } else if !out.ends_with([' ', '\n', '\r', '\t']) {
                out.push(' ');
            }
        }
        pending.clear();
        match &piece.token {
            Token::Word(w)
                if case.is_some() && w.quote_style.is_none() && w.keyword != Keyword::NoKeyword =>
            {
                if case == Some(true) {
                    out.push_str(&piece.raw.to_ascii_uppercase());
                } else {
                    out.push_str(&piece.raw.to_ascii_lowercase());
                }
            }
            _ => out.push_str(piece.raw),
        }
        match piece.token {
            Token::LParen => depth = depth.saturating_add(1),
            Token::RParen => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    out
}

/// Turn clipboard tab/newline cells into a parenthesized list of quoted text,
/// never expressions. Spaces and empty cells are data; one final row terminator
/// is ignored. Backslashes are doubled for MySQL's default string mode, quotes
/// are doubled for both standard SQL and MySQL. No SQL expression is evaluated.
pub fn in_condition(text: &str) -> Result<String> {
    ensure!(
        text.len() <= MAX_SQL_BYTES,
        "Clipboard SQL exceeds the 64 KiB limit"
    );
    ensure!(
        !text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\t' | '\r' | '\n')),
        "Clipboard contains unsupported control characters"
    );
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let normalized = normalized.strip_suffix('\n').unwrap_or(&normalized);
    let mut out = String::from("(");
    for (i, cell) in normalized.split(['\t', '\n']).enumerate() {
        if i != 0 {
            out.push_str(", ");
        }
        out.push('\'');
        for c in cell.chars() {
            if c == '\\' || c == '\'' {
                out.push(c);
            }
            out.push(c);
        }
        out.push('\'');
        ensure!(
            out.len() < MAX_SQL_BYTES,
            "Quoted clipboard SQL exceeds the 64 KiB limit"
        );
    }
    out.push(')');
    Ok(out)
}

fn check_path(path: &Path, new: bool) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.components().any(|c| matches!(c, Component::ParentDir)),
        "SQL path must not contain parent-directory traversal"
    );
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match fs::symlink_metadata(ancestor) {
            Ok(m) => {
                ensure!(
                    !m.file_type().is_symlink(),
                    "SQL paths must not contain symlinks"
                );
                if ancestor == path {
                    ensure!(
                        m.is_file() && !new,
                        "SQL destination must be a new file; existing files are never overwritten"
                    );
                } else {
                    ensure!(m.is_dir(), "SQL parent path must be a directory");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && ancestor == path && new => {}
            Err(_) => bail!("Cannot inspect SQL path; check directory and file permissions"),
        }
    }
    Ok(())
}

/// Kit normalizes newlines globally; refuse file/tool text whose literal bytes
/// would change rather than silently changing SQL string data.
pub fn contains_literal_carriage_return(sql: &str) -> bool {
    if !sql.contains('\r') {
        return false;
    }
    let Some(pieces) = pieces(sql) else {
        return true;
    };
    pieces
        .iter()
        .any(|p| !gap(&p.token) && p.raw.contains('\r'))
}

pub fn read_sql(path: &Path) -> Result<String> {
    check_path(path, false)?;
    let file = File::open(path).map_err(|_| anyhow::anyhow!("Cannot open SQL file"))?;
    ensure!(
        file.metadata()
            .map_err(|_| anyhow::anyhow!("Cannot inspect SQL file"))?
            .len()
            <= MAX_SQL_BYTES as u64,
        "SQL file exceeds the 64 KiB limit"
    );
    let mut bytes = Vec::new();
    file.take(MAX_SQL_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Cannot read SQL file"))?;
    ensure!(
        bytes.len() <= MAX_SQL_BYTES,
        "SQL file exceeds the 64 KiB limit"
    );
    let text = std::str::from_utf8(&bytes).map_err(|_| anyhow::anyhow!("SQL file is not UTF-8"))?;
    ensure!(
        !contains_literal_carriage_return(text),
        "SQL contains literal carriage returns that the editor cannot preserve"
    );
    Ok(text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n")
        .replace('\r', "\n"))
}

/// Publish a complete private file atomically without ever overwriting a target.
pub fn write_sql(path: &Path, text: &str) -> Result<()> {
    ensure!(
        text.len() <= MAX_SQL_BYTES,
        "SQL file exceeds the 64 KiB limit"
    );
    check_path(path, true)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".dalan-sql-{}.tmp", Uuid::new_v4()));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| anyhow::anyhow!("Cannot create SQL temporary file"))?;
    let _cleanup = Cleanup(temporary.clone());
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| anyhow::anyhow!("Cannot write SQL file"))?;
    drop(file);
    check_path(path, true)?;
    fs::hard_link(&temporary, path).map_err(|_| anyhow::anyhow!("Cannot publish SQL file; destination must be new and filesystem must support hard links"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("dalan-sql-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn literal_carriage_returns_are_rejected_before_editor_normalization() {
        assert!(contains_literal_carriage_return("SELECT 'a\rb'"));
        assert!(contains_literal_carriage_return("SELECT 'a\r\nb'"));
        assert!(!contains_literal_carriage_return("SELECT 1;\r\n"));
    }

    #[test]
    fn case_only_keywords_and_clause_lines() {
        let sql = "select CustomerName, `order`, \"Mixed\" from Customers where CustomerKey = 1 order by CustomerKey";
        let expected = "SELECT CustomerName, `order`, \"Mixed\"\nFROM Customers\nWHERE CustomerKey = 1\nORDER BY CustomerKey";
        assert_eq!(format_sql(sql, true), expected);
        assert!(format_sql(expected, false).starts_with("select CustomerName"));
    }
    #[test]
    fn literal_spelling_is_exact() {
        let sql = r"select 'a  b', 'it''s', 'a\'b', 'c\\d' from Names";
        let formatted = format_sql(sql, true);
        assert!(formatted.contains(r"'a  b', 'it''s', 'a\'b', 'c\\d'"));
        assert!(compress_sql(sql).contains(r"'a  b', 'it''s', 'a\'b', 'c\\d'"));
    }
    #[test]
    fn comments_keep_content_and_cannot_swallow_statement() {
        let sql = "select 1 -- keep  exact\n  from T; # next  query\n select 2 /* block  Text */";
        for out in [format_sql(sql, true), compress_sql(sql)] {
            assert!(out.contains("-- keep  exact\n"));
            assert!(out.contains("# next  query\n"));
            assert!(out.contains("/* block  Text */"));
            assert!(out.contains("2 /* block  Text */"));
        }
    }
    #[test]
    fn mysql_dash_comment_rule_does_not_merge() {
        assert_eq!(
            compress_sql("select  1--2, 1 - - 2"),
            "select 1--2, 1 - - 2"
        );
        assert_eq!(
            compress_sql("select 1 -- comment\n select 2"),
            "select 1 -- comment\nselect 2"
        );
    }
    #[test]
    fn executable_comments_and_hints_unchanged() {
        for sql in [
            "select /*!40101 SQL_NO_CACHE */  1",
            "/*M!100100 select 1 */",
            "select /*+ INDEX(x) */ 1",
        ] {
            assert_eq!(format_sql(sql, true), sql);
            assert_eq!(compress_sql(sql), sql);
        }
    }
    #[test]
    fn malformed_and_unsupported_retained() {
        for sql in [
            "select 'unfinished",
            "select `unfinished",
            "select /* unfinished",
            "select \0secret",
            "select [select] from [order]",
            "select $$select from string$$",
        ] {
            assert_eq!(format_sql(sql, true), sql);
            assert_eq!(compress_sql(sql), sql);
        }
    }
    #[test]
    fn unicode_and_limits() {
        assert_eq!(
            format_sql("select café, '東京' from 数据", true),
            "SELECT café, '東京'\nFROM 数据"
        );
        let large = "s".repeat(MAX_SQL_BYTES + 1);
        assert_eq!(format_sql(&large, true), large);
        assert_eq!(compress_sql(&large), large);
        assert!(in_condition(&large).is_err());
        assert!(in_condition(&"'".repeat(MAX_SQL_BYTES / 2)).is_err());
    }
    #[test]
    fn transforms_are_idempotent() {
        for sql in [
            "select  x from  t where x=1",
            "select ( select x from t ) from u",
            "select 1 -- a\n from t",
            " select 'x'\n'y' ",
            "\n",
            "",
        ] {
            for uppercase in [true, false] {
                let once = format_sql(sql, uppercase);
                assert_eq!(format_sql(&once, uppercase), once);
            }
            let once = compress_sql(sql);
            assert_eq!(compress_sql(&once), once);
        }
    }
    #[test]
    fn compressor_retains_newline_literal_boundary_and_adjacent_tokens() {
        assert_eq!(
            compress_sql("select  'a'\n   'b', a::text, 1+2"),
            "select 'a'\n'b', a::text, 1+2"
        );
    }
    #[test]
    fn clipboard_is_quoted_text_not_expressions() {
        assert_eq!(
            in_condition("1\tNULL\r\n'); DROP TABLE x; --\n").unwrap(),
            "('1', 'NULL', '''); DROP TABLE x; --')"
        );
        assert_eq!(
            in_condition("a\\b\tO'Reilly\t\n").unwrap(),
            "('a\\\\b', 'O''Reilly', '')"
        );
        assert_eq!(in_condition(" a \t\t").unwrap(), "(' a ', '', '')");
        assert_eq!(in_condition("").unwrap(), "('')");
        assert!(in_condition("a\0b").is_err());
    }
    #[test]
    fn file_roundtrip_bom_cr_private_and_no_overwrite() {
        let dir = Temp::new();
        let path = dir.0.join("query.sql");
        write_sql(&path, "\u{feff}select 'café'\r\nfrom T\rwhere x=1").unwrap();
        assert_eq!(read_sql(&path).unwrap(), "select 'café'\nfrom T\nwhere x=1");
        assert!(write_sql(&path, "replacement").is_err());
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn file_bounds_invalid_utf8_and_sanitized_errors() {
        let dir = Temp::new();
        let path = dir.0.join("sensitive-file-name.sql");
        assert!(
            !read_sql(&path)
                .unwrap_err()
                .to_string()
                .contains("sensitive")
        );
        fs::write(&path, [0xff]).unwrap();
        assert!(read_sql(&path).unwrap_err().to_string().contains("UTF-8"));
        fs::write(&path, "a".repeat(MAX_SQL_BYTES + 1)).unwrap();
        assert!(read_sql(&path).is_err());
        let exact = dir.0.join("exact.sql");
        write_sql(&exact, &"a".repeat(MAX_SQL_BYTES)).unwrap();
        assert_eq!(read_sql(&exact).unwrap().len(), MAX_SQL_BYTES);
        assert!(write_sql(&dir.0.join("large.sql"), &"a".repeat(MAX_SQL_BYTES + 1)).is_err());
        assert!(read_sql(&dir.0.join("../query.sql")).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_files_and_ancestors() {
        use std::os::unix::fs::symlink;
        let dir = Temp::new();
        let real = dir.0.join("real.sql");
        write_sql(&real, "select 1").unwrap();
        let link = dir.0.join("link.sql");
        symlink(&real, &link).unwrap();
        assert!(read_sql(&link).is_err());
        assert!(write_sql(&link, "select 2").is_err());
        let parent = dir.0.join("linked");
        symlink(&dir.0, &parent).unwrap();
        assert!(read_sql(&parent.join("real.sql")).is_err());
        assert!(write_sql(&parent.join("new.sql"), "select 2").is_err());
    }
}
