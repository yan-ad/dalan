//! CSV export of the currently loaded page, without fetching additional rows.
//!
//! Non-NULL fields (including numbers) are quoted, with RFC 4180 escaping and
//! CRLF record separators. NULL is the unquoted marker `\N`; real text `\N`
//! remains quoted. This distinction is syntactic: ordinary CSV readers often
//! discard quoting information and do not provide NULL semantics automatically.
//! Binary and temporal cells use their loaded display strings, not raw database
//! values. Truncated previews are rejected rather than exported as complete data.
//!
//! Spreadsheet protection prefixes risky strings with an apostrophe by default.
//! This intentionally changes those values; disable it only for trusted data.
//! Quoting alone does not prevent formula execution, nor does it guarantee that a
//! spreadsheet will preserve large integers or other numeric formatting.

use anyhow::{Context, Result, ensure};
use dalan_drivers::{CellValue, TablePage};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use uuid::Uuid;

const MAX_COLUMNS: usize = 512;
const MAX_ROWS: usize = 200;
const MAX_EXPORT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    pub spreadsheet_safe: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            spreadsheet_safe: true,
        }
    }
}

/// Encode only the loaded rows (even when `has_more` is true).
///
/// Rejects truncated values, malformed row widths, and pages exceeding the
/// preview/export bounds. See the module documentation for NULL and spreadsheet
/// semantics. Output is UTF-8 without a BOM and includes a header record.
pub fn encode_loaded_page(page: &TablePage, options: &ExportOptions) -> Result<Vec<u8>> {
    ensure!(
        !page.truncated,
        "Loaded values are truncated; cannot export them as complete data"
    );
    ensure!(
        page.columns.len() <= MAX_COLUMNS,
        "Loaded page exceeds the 512 column limit"
    );
    ensure!(
        page.rows.len() <= MAX_ROWS,
        "Loaded page exceeds the 200 row limit"
    );
    ensure!(
        page.rows.iter().all(|row| row.len() == page.columns.len()),
        "Loaded page row width does not match its columns"
    );
    let mut output = Vec::new();
    for (index, column) in page.columns.iter().enumerate() {
        if index != 0 {
            append(&mut output, b",")?;
        }
        quoted(&mut output, &column.name, options.spreadsheet_safe)?;
    }
    append(&mut output, b"\r\n")?;
    for row in &page.rows {
        for (index, cell) in row.iter().enumerate() {
            if index != 0 {
                append(&mut output, b",")?;
            }
            match cell {
                CellValue::Null => append(&mut output, b"\\N")?,
                CellValue::Number(value) => quoted(
                    &mut output,
                    value,
                    options.spreadsheet_safe && !is_decimal(value),
                )?,
                CellValue::Text(value) | CellValue::Binary(value) | CellValue::Temporal(value) => {
                    quoted(&mut output, value, options.spreadsheet_safe)?;
                }
            }
        }
        append(&mut output, b"\r\n")?;
    }
    Ok(output)
}

fn append(output: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() <= MAX_EXPORT_BYTES - output.len(),
        "CSV export exceeds the 8 MiB limit"
    );
    output.extend_from_slice(bytes);
    Ok(())
}

fn quoted(output: &mut Vec<u8>, value: &str, safe: bool) -> Result<()> {
    let prefix = safe && formula_risk(value);
    // Check before allocating or copying a potentially fabricated, oversized cell.
    let length = value
        .len()
        .checked_add(value.bytes().filter(|&b| b == b'"').count())
        .and_then(|n| n.checked_add(2 + usize::from(prefix)));
    ensure!(
        length.is_some_and(|n| n <= MAX_EXPORT_BYTES - output.len()),
        "CSV export exceeds the 8 MiB limit"
    );
    output.push(b'"');
    if prefix {
        output.push(b'\'');
    }
    for byte in value.bytes() {
        output.push(byte);
        if byte == b'"' {
            output.push(b'"');
        }
    }
    output.push(b'"');
    Ok(())
}

fn formula_risk(value: &str) -> bool {
    for ch in value.chars() {
        // These controls are risky even before otherwise benign text.
        if matches!(ch, '\t' | '\r') {
            return true;
        }
        if !ch.is_whitespace() {
            return matches!(ch, '=' | '+' | '-' | '@');
        }
    }
    false
}

// Lexical validation only: no conversion/rounding, and arbitrarily large decimal
// numbers remain exact. Unexpected Number fixtures receive normal protection.
fn is_decimal(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut i = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let mut digits = 0;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        digits += 1;
        i += 1;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 {
        return false;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == bytes.len()
}

/// Encode and atomically publish to a **new** destination; never overwrite a file
/// or symlink. Existing destinations require the caller to choose another name.
///
/// A private staging file in the destination directory is synced before a hard
/// link publishes it. Filesystems without hard-link support return an error.
/// Unix files are created with mode 0600 (other platforms use their native ACLs).
/// Portable path operations do not protect against hostile directory replacement.
/// A staging cleanup error after publication is reported, but the complete export
/// remains at the destination. No file is created if encoding fails.
pub fn export_loaded_page(path: &Path, page: &TablePage, options: &ExportOptions) -> Result<()> {
    let bytes = encode_loaded_page(page, options)?;
    ensure!(
        path.file_name().is_some(),
        "CSV destination must name a file"
    );
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".dalan-export-{}.tmp", Uuid::new_v4()));
    let mut open = OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    let mut file = open
        .open(&temporary)
        .context("Cannot create CSV staging file")?;
    let result: Result<()> = (|| {
        file.write_all(&bytes).context("Cannot write CSV export")?;
        file.sync_all().context("Cannot sync CSV export")?;
        fs::hard_link(&temporary, path).context(
            "Cannot publish CSV export; choose a new filename and a filesystem supporting hard links",
        )?;
        Ok(())
    })();
    drop(file);
    let cleanup = fs::remove_file(&temporary).context("Cannot remove CSV staging file");
    result?;
    cleanup
}

#[cfg(test)]
mod tests {
    use super::*;
    use dalan_drivers::ColumnInfo;

    fn page(names: &[&str], rows: Vec<Vec<CellValue>>) -> TablePage {
        TablePage {
            columns: names
                .iter()
                .map(|name| ColumnInfo {
                    name: (*name).into(),
                    data_type: "text".into(),
                    nullable: true,
                    is_primary_key: false,
                })
                .collect(),
            rows,
            has_more: true,
            next_offset: Some(100),
            offset: 99,
            truncated: false,
        }
    }

    fn encode(page: &TablePage) -> String {
        String::from_utf8(encode_loaded_page(page, &ExportOptions::default()).unwrap()).unwrap()
    }

    #[test]
    fn escapes_utf8_and_preserves_null_distinction() {
        let p = page(
            &["a,\"b", "空", "null", "empty", "marker"],
            vec![vec![
                CellValue::Text("hello,\"world\"\nnext\r\nlast".into()),
                CellValue::Text("雪 🦀".into()),
                CellValue::Null,
                CellValue::Text(String::new()),
                CellValue::Text("\\N".into()),
            ]],
        );
        assert_eq!(
            encode(&p),
            "\"a,\"\"b\",\"空\",\"null\",\"empty\",\"marker\"\r\n\"hello,\"\"world\"\"\nnext\r\nlast\",\"雪 🦀\",\\N,\"\",\"\\N\"\r\n"
        );
    }

    #[test]
    fn protects_headers_and_all_display_types_but_preserves_numbers() {
        let p = page(
            &["=header", "b", "c", "d", "e", "f"],
            vec![vec![
                CellValue::Text("\u{2003}=SUM(1)".into()),
                CellValue::Binary("+cmd".into()),
                CellValue::Temporal("-12:00".into()),
                CellValue::Number("-12345678901234567890.00e+2".into()),
                CellValue::Number("=evil".into()),
                CellValue::Text(" \tbenign".into()),
            ]],
        );
        assert_eq!(
            encode(&p),
            "\"'=header\",\"b\",\"c\",\"d\",\"e\",\"f\"\r\n\"'\u{2003}=SUM(1)\",\"'+cmd\",\"'-12:00\",\"-12345678901234567890.00e+2\",\"'=evil\",\"' \tbenign\"\r\n"
        );
        let raw = String::from_utf8(
            encode_loaded_page(
                &p,
                &ExportOptions {
                    spreadsheet_safe: false,
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert!(raw.starts_with("\"=header\""));
        assert!(raw.contains("\"=evil\""));
        for s in ["=x", "+x", "-x", "@x", "\tfoo", "\rfoo", " \n@x"] {
            assert!(formula_risk(s), "{s:?}");
        }
    }

    #[test]
    fn decimal_validation_does_not_change_formatting() {
        for s in [
            "0",
            "-0",
            "+1",
            ".5",
            "1.",
            "1e-99999",
            "123456789012345678901234567890",
        ] {
            assert!(is_decimal(s), "{s}");
        }
        for s in ["", "-", "NaN", "inf", "1e", "1e+", "1+2", " 1", "=1", "1\r"] {
            assert!(!is_decimal(s), "{s:?}");
        }
    }

    #[test]
    fn empty_loaded_page_is_headers_only() {
        assert_eq!(encode(&page(&["a", "b"], vec![])), "\"a\",\"b\"\r\n");
    }

    #[test]
    fn rejects_truncation_mismatches_and_limits() {
        let mut p = page(&["a"], vec![vec![]]);
        assert!(encode_loaded_page(&p, &ExportOptions::default()).is_err());
        p.rows.clear();
        p.truncated = true;
        assert!(
            encode_loaded_page(&p, &ExportOptions::default())
                .unwrap_err()
                .to_string()
                .contains("Loaded values are truncated")
        );
        p.truncated = false;
        p.rows = vec![vec![CellValue::Null]; 201];
        assert!(encode_loaded_page(&p, &ExportOptions::default()).is_err());
        p.rows.clear();
        p.columns = vec![p.columns[0].clone(); 513];
        assert!(encode_loaded_page(&p, &ExportOptions::default()).is_err());
        let p = page(
            &["a"],
            vec![vec![CellValue::Text("x".repeat(MAX_EXPORT_BYTES))]],
        );
        assert!(encode_loaded_page(&p, &ExportOptions::default()).is_err());
    }

    struct TestDirectory(std::path::PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("dalan-export-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn publishes_private_file_without_overwrite_or_staging_residue() {
        let dir = TestDirectory::new();
        let path = dir.0.join("export.csv");
        let p = page(&["a"], vec![vec![CellValue::Text("value".into())]]);
        export_loaded_page(&path, &p, &ExportOptions::default()).unwrap();
        assert_eq!(fs::read(&path).unwrap(), encode(&p).as_bytes());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::write(&path, b"keep").unwrap();
        assert!(export_loaded_page(&path, &p, &ExportOptions::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        let mut truncated = p.clone();
        truncated.truncated = true;
        assert!(
            export_loaded_page(
                &dir.0.join("bad.csv"),
                &truncated,
                &ExportOptions::default()
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_existing_symlink_including_dangling_symlink() {
        use std::os::unix::fs::symlink;
        let dir = TestDirectory::new();
        let target = dir.0.join("target");
        let link = dir.0.join("export.csv");
        let p = page(&["a"], vec![]);
        symlink(&target, &link).unwrap();
        assert!(export_loaded_page(&link, &p, &ExportOptions::default()).is_err());
        assert!(!target.exists());
        fs::write(&target, b"keep").unwrap();
        assert!(export_loaded_page(&link, &p, &ExportOptions::default()).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
    }
}
