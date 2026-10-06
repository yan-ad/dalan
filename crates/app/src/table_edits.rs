//! Pure, bounded edits over a shared, immutable loaded page. No database I/O.
//! Callers must also gate the engine and table kind (ordinary MySQL/PG tables).
//! `None` in an inserted row means DEFAULT, not SQL NULL. Row indices of inserts
//! follow the base rows; removing an insert shifts subsequent insert indices.

use anyhow::{Result, anyhow, ensure};
use dalan_drivers::{CellValue, ColumnInfo, RowMutation, TablePage, WriteRequest};
use std::{collections::BTreeMap, sync::Arc};

const MAX_OPERATIONS: usize = 100;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_CELL_BYTES: usize = 65536;

#[derive(Debug, Clone, Default)]
struct RowEdit {
    changes: BTreeMap<usize, CellValue>,
    deleted: bool,
}

#[derive(Debug, Clone)]
pub struct TableEdits {
    base: Arc<TablePage>,
    database: String,
    table: String,
    entries: BTreeMap<usize, RowEdit>,
    inserts: Vec<Vec<Option<CellValue>>>,
}

/// Preview-level eligibility only; backend metadata is revalidated on save.
pub fn can_edit(page: &TablePage) -> Result<()> {
    ensure!(!page.truncated, "Truncated previews cannot be edited");
    ensure!(
        (1..=512).contains(&page.columns.len()),
        "Invalid column count"
    );
    ensure!(
        page.columns.iter().any(|c| c.is_primary_key),
        "Editing requires a primary key"
    );
    ensure!(
        page.columns
            .iter()
            .filter(|c| c.is_primary_key)
            .all(|c| !c.nullable),
        "Editing requires a non-nullable primary key"
    );
    for c in &page.columns {
        ensure!(
            !c.data_type.to_ascii_lowercase().starts_with("numeric")
                && !c.data_type.to_ascii_lowercase().starts_with("decimal"),
            "Decimal writes require precision validation and are unavailable"
        );
        column_kind(c)?;
    }
    for row in &page.rows {
        ensure!(row.len() == page.columns.len(), "Incomplete loaded row");
        for (v, c) in row.iter().zip(&page.columns) {
            validate_value(v, c)?;
            ensure!(
                !c.is_primary_key || !matches!(v, CellValue::Null),
                "Loaded primary key is NULL"
            );
        }
    }
    Ok(())
}

impl TableEdits {
    pub fn new(page: Arc<TablePage>, database: String, table: String) -> Result<Self> {
        can_edit(&page)?;
        ensure!(
            !database.is_empty()
                && database.len() <= 256
                && !database.contains('\0')
                && !table.is_empty()
                && table.len() <= 1024
                && !table.contains('\0'),
            "Invalid write table identifier"
        );
        Ok(Self {
            base: page,
            database,
            table,
            entries: BTreeMap::new(),
            inserts: Vec::new(),
        })
    }

    /// Exact shared snapshot identity; originals are never copied until request().
    pub fn base(&self) -> &Arc<TablePage> {
        &self.base
    }
    pub fn rows_count(&self) -> usize {
        self.base.rows.len() + self.inserts.len()
    }
    pub fn is_insert(&self, row: usize) -> bool {
        row >= self.base.rows.len() && row < self.rows_count()
    }
    pub fn is_deleted(&self, row: usize) -> bool {
        self.entries.get(&row).is_some_and(|e| e.deleted)
    }
    pub fn is_changed(&self, row: usize, col: usize) -> bool {
        if self.is_insert(row) {
            return self.displayed(row, col).is_some();
        }
        self.entries
            .get(&row)
            .is_some_and(|e| e.changes.contains_key(&col))
    }
    /// None means DEFAULT for a valid inserted cell; also returned for invalid indices.
    pub fn displayed(&self, row: usize, col: usize) -> Option<&CellValue> {
        if row >= self.base.rows.len() {
            return self
                .inserts
                .get(row.checked_sub(self.base.rows.len())?)?
                .get(col)?
                .as_ref();
        }
        self.entries
            .get(&row)
            .and_then(|e| e.changes.get(&col))
            .or_else(|| self.base.rows.get(row)?.get(col))
    }
    pub fn inserted_values(&self, row: usize) -> Option<&[Option<CellValue>]> {
        self.inserts
            .get(row.checked_sub(self.base.rows.len())?)
            .map(Vec::as_slice)
    }
    pub fn operation_count(&self) -> usize {
        self.entries.len() + self.inserts.len()
    }
    pub fn has_changes(&self) -> bool {
        self.operation_count() != 0
    }

    pub fn set_cell(&mut self, row: usize, col: usize, value: CellValue) -> Result<()> {
        self.indices(row, col)?;
        ensure!(
            !self.is_deleted(row),
            "Restore the deleted row before editing it"
        );
        validate_value(&value, &self.base.columns[col])?;
        self.change(|s| {
            if row >= s.base.rows.len() {
                s.inserts[row - s.base.rows.len()][col] = Some(value);
            } else {
                if value == s.base.rows[row][col] {
                    if let Some(e) = s.entries.get_mut(&row) {
                        e.changes.remove(&col);
                    }
                    s.prune(row);
                } else {
                    s.entries.entry(row).or_default().changes.insert(col, value);
                }
            }
        })
    }
    pub fn set_null(&mut self, row: usize, col: usize) -> Result<()> {
        self.set_cell(row, col, CellValue::Null)
    }
    /// Omit an insert field, leaving database defaults to the backend.
    pub fn set_default(&mut self, row: usize, col: usize) -> Result<()> {
        self.indices(row, col)?;
        ensure!(
            self.is_insert(row),
            "DEFAULT is only available for inserted rows"
        );
        self.inserts[row - self.base.rows.len()][col] = None;
        Ok(())
    }
    pub fn delete_row(&mut self, row: usize) -> Result<()> {
        ensure!(row < self.rows_count(), "Row is outside the loaded page");
        self.change(|s| {
            if row >= s.base.rows.len() {
                s.inserts.remove(row - s.base.rows.len());
            } else {
                s.entries.entry(row).or_default().deleted = true;
            }
        })
    }
    /// Restores a base row while retaining any cell edits made before deletion.
    pub fn restore_row(&mut self, row: usize) -> Result<()> {
        ensure!(
            row < self.base.rows.len(),
            "Only loaded rows can be restored"
        );
        self.change(|s| {
            if let Some(e) = s.entries.get_mut(&row) {
                e.deleted = false;
            }
            s.prune(row);
        })
    }
    pub fn add_row(&mut self) -> Result<usize> {
        let row = self.rows_count();
        self.change(|s| s.inserts.push(vec![None; s.base.columns.len()]))?;
        Ok(row)
    }
    /// Clone displayed values, omitting every primary-key field for regeneration.
    pub fn clone_row(&mut self, row: usize) -> Result<usize> {
        ensure!(
            row < self.rows_count() && !self.is_deleted(row),
            "Cannot clone this row"
        );
        let values = self
            .base
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if c.is_primary_key {
                    None
                } else {
                    self.displayed(row, i).cloned()
                }
            })
            .collect();
        let index = self.rows_count();
        self.change(|s| s.inserts.push(values))?;
        Ok(index)
    }
    pub fn discard(&mut self) {
        self.entries.clear();
        self.inserts.clear();
    }

    /// Materialize complete original rows only at the explicit save boundary.
    pub fn request(&self) -> Result<WriteRequest> {
        ensure!(self.has_changes(), "No staged changes");
        self.check_bounds()?;
        let mut mutations = Vec::with_capacity(self.operation_count());
        for (&row, edit) in &self.entries {
            let original = self.base.rows[row].clone();
            mutations.push(if edit.deleted {
                RowMutation::Delete { original }
            } else {
                RowMutation::Update {
                    original,
                    changes: edit.changes.iter().map(|(&i, v)| (i, v.clone())).collect(),
                }
            });
        }
        for row in &self.inserts {
            mutations.push(RowMutation::Insert {
                values: row
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| v.clone().map(|v| (i, v)))
                    .collect(),
            });
        }
        Ok(WriteRequest {
            database: self.database.clone(),
            table: self.table.clone(),
            expected_columns: self.base.columns.clone(),
            mutations,
        })
    }
    fn indices(&self, row: usize, col: usize) -> Result<()> {
        ensure!(
            row < self.rows_count() && col < self.base.columns.len(),
            "Cell is outside the loaded page"
        );
        Ok(())
    }
    fn prune(&mut self, row: usize) {
        if self
            .entries
            .get(&row)
            .is_some_and(|e| !e.deleted && e.changes.is_empty())
        {
            self.entries.remove(&row);
        }
    }
    // Transactional staging: errors leave the visible state exactly unchanged.
    fn change(&mut self, f: impl FnOnce(&mut Self)) -> Result<()> {
        let entries = self.entries.clone();
        let inserts = self.inserts.clone();
        f(self);
        if let Err(e) = self.check_bounds() {
            self.entries = entries;
            self.inserts = inserts;
            return Err(e);
        }
        Ok(())
    }
    fn check_bounds(&self) -> Result<()> {
        ensure!(
            self.operation_count() <= MAX_OPERATIONS,
            "At most 100 row operations may be staged"
        );
        let mut bytes = 0usize;
        for (&row, edit) in &self.entries {
            for v in &self.base.rows[row] {
                bytes += cell_bytes(v)?;
            }
            // Retained pre-delete changes count too, bounding local staging memory.
            for v in edit.changes.values() {
                bytes += cell_bytes(v)?;
            }
        }
        for row in &self.inserts {
            for v in row.iter().flatten() {
                bytes += cell_bytes(v)?;
            }
        }
        ensure!(bytes <= MAX_BYTES, "Staged changes exceed 2 MiB");
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Text,
    Boolean,
    Integer { min: i128, max: i128 },
    Decimal,
}

fn column_kind(column: &ColumnInfo) -> Result<Kind> {
    let t = column.data_type.trim().to_ascii_lowercase();
    let unsigned = t.ends_with(" unsigned");
    let t = t.strip_suffix(" unsigned").unwrap_or(&t);
    let (name, modifier) =
        match t.split_once('(') {
            Some((name, suffix)) => {
                let inner = suffix
                    .strip_suffix(')')
                    .ok_or_else(|| anyhow!("Unsupported column type"))?;
                ensure!(
                    !inner.is_empty()
                        && inner.split(',').all(|p| !p.trim().is_empty()
                            && p.trim().bytes().all(|b| b.is_ascii_digit())),
                    "Unsupported column type modifier"
                );
                (name.trim(), true)
            }
            None => (t, false),
        };
    let bits = match name {
        "tinyint" => Some(8),
        "smallint" | "int2" => Some(16),
        "mediumint" => Some(24),
        "int" | "integer" | "int4" => Some(32),
        "bigint" | "int8" => Some(64),
        _ => None,
    };
    if let Some(bits) = bits {
        return Ok(if unsigned {
            Kind::Integer {
                min: 0,
                max: (1i128 << bits) - 1,
            }
        } else {
            Kind::Integer {
                min: -(1i128 << (bits - 1)),
                max: (1i128 << (bits - 1)) - 1,
            }
        });
    }
    ensure!(!unsigned, "Unsupported unsigned column type");
    match name {
        "text" | "tinytext" | "mediumtext" | "longtext" if !modifier => Ok(Kind::Text),
        "char" | "varchar" | "character" | "character varying" | "bpchar" => Ok(Kind::Text),
        "bool" | "boolean" if !modifier => Ok(Kind::Boolean),
        "numeric" | "decimal" => Ok(Kind::Decimal),
        _ => Err(anyhow!(
            "Table contains unsupported or lossy write column types"
        )),
    }
}

/// Parse a typed cell. Empty text stays empty text; NULL is a separate action.
/// Decimal input never passes through floating point; exponents are rejected.
pub fn parse_cell(text: &str, column: &ColumnInfo) -> Result<CellValue> {
    let value = match column_kind(column)? {
        Kind::Text => CellValue::Text(text.into()),
        Kind::Boolean => {
            ensure!(
                matches!(text, "true" | "false"),
                "Boolean must be true or false"
            );
            CellValue::Text(text.into())
        }
        Kind::Integer { min, max } => {
            ensure!(decimal(text, true), "Enter a whole decimal integer");
            let n: i128 = text
                .parse()
                .map_err(|_| anyhow!("Integer is out of range"))?;
            ensure!((min..=max).contains(&n), "Integer is out of range");
            CellValue::Number(n.to_string())
        }
        Kind::Decimal => {
            ensure!(decimal(text, false), "Enter an exact decimal number");
            CellValue::Number(text.into())
        }
    };
    cell_bytes(&value)?;
    Ok(value)
}
fn decimal(s: &str, integer: bool) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let mut p = s.split('.');
    let a = p.next().unwrap_or("");
    let b = p.next();
    !a.is_empty()
        && a.bytes().all(|b| b.is_ascii_digit())
        && p.next().is_none()
        && b.is_none_or(|b| !integer && !b.is_empty() && b.bytes().all(|b| b.is_ascii_digit()))
}
fn cell_bytes(value: &CellValue) -> Result<usize> {
    let n = match value {
        CellValue::Null => 0,
        CellValue::Text(s) | CellValue::Number(s) => {
            ensure!(!s.contains('\0'), "Write values cannot contain NUL");
            s.len()
        }
        _ => return Err(anyhow!("Binary and temporal writes are not supported")),
    };
    ensure!(n <= MAX_CELL_BYTES, "Write value exceeds 64 KiB");
    Ok(n)
}
fn validate_value(value: &CellValue, column: &ColumnInfo) -> Result<()> {
    column_kind(column)?;
    cell_bytes(value)?;
    if matches!(value, CellValue::Null) {
        ensure!(column.nullable, "Column does not allow NULL");
        return Ok(());
    }
    match (column_kind(column)?, value) {
        (Kind::Text | Kind::Boolean, CellValue::Text(s))
        | (Kind::Integer { .. } | Kind::Decimal, CellValue::Number(s)) => {
            parse_cell(s, column)?;
            Ok(())
        }
        _ => Err(anyhow!("Value does not match column type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn column(name: &str, ty: &str, nullable: bool, pk: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.into(),
            data_type: ty.into(),
            nullable,
            is_primary_key: pk,
        }
    }
    fn page() -> Arc<TablePage> {
        Arc::new(TablePage {
            columns: vec![
                column("id", "integer", false, true),
                column("name", "text", true, false),
            ],
            rows: vec![vec![
                CellValue::Number("1".into()),
                CellValue::Text("a".into()),
            ]],
            has_more: false,
            next_offset: None,
            offset: 0,
            truncated: false,
        })
    }
    fn edits() -> TableEdits {
        TableEdits::new(page(), "db".into(), "table".into()).unwrap()
    }
    #[test]
    fn no_op_and_discard_keep_shared_base() {
        let p = page();
        let mut e = TableEdits::new(p.clone(), "db".into(), "t".into()).unwrap();
        assert!(Arc::ptr_eq(e.base(), &p));
        e.set_cell(0, 1, CellValue::Text("a".into())).unwrap();
        assert!(!e.has_changes());
        e.set_cell(0, 1, CellValue::Text("b".into())).unwrap();
        assert_eq!(p.rows[0][1], CellValue::Text("a".into()));
        e.set_cell(0, 1, CellValue::Text("a".into())).unwrap();
        assert!(!e.has_changes());
        e.add_row().unwrap();
        e.discard();
        assert_eq!(e.rows_count(), 1);
        assert!(e.request().is_err());
    }
    #[test]
    fn null_empty_default_are_distinct() {
        let mut e = edits();
        let i = e.add_row().unwrap();
        assert_eq!(e.displayed(i, 1), None);
        e.set_cell(i, 1, parse_cell("", &e.base.columns[1]).unwrap())
            .unwrap();
        assert_eq!(e.displayed(i, 1), Some(&CellValue::Text("".into())));
        e.set_null(i, 1).unwrap();
        assert_eq!(e.displayed(i, 1), Some(&CellValue::Null));
        e.set_default(i, 1).unwrap();
        assert_eq!(e.displayed(i, 1), None);
        assert!(e.set_null(i, 0).is_err());
        assert!(e.set_default(0, 1).is_err());
    }
    #[test]
    fn clone_omits_keys_and_delete_restore_retains_edits() {
        let mut e = edits();
        e.set_cell(0, 1, CellValue::Text("edited".into())).unwrap();
        let i = e.clone_row(0).unwrap();
        assert_eq!(e.displayed(i, 0), None);
        assert_eq!(e.displayed(i, 1), Some(&CellValue::Text("edited".into())));
        e.delete_row(0).unwrap();
        assert!(e.is_deleted(0));
        assert!(e.set_null(0, 1).is_err());
        assert!(
            matches!(&e.request().unwrap().mutations[0], RowMutation::Delete { original } if original[1] == CellValue::Text("a".into()))
        );
        e.restore_row(0).unwrap();
        assert!(e.is_changed(0, 1));
        e.delete_row(i).unwrap();
        assert_eq!(e.operation_count(), 1);
    }
    #[test]
    fn exact_types_and_bounds() {
        for (ty, yes, no) in [
            ("smallint", "32767", "32768"),
            ("int4", "-2147483648", "2147483648"),
            ("bigint", "9223372036854775807", "9223372036854775808"),
            ("tinyint unsigned", "255", "-1"),
        ] {
            let c = column("n", ty, false, false);
            assert!(parse_cell(yes, &c).is_ok());
            assert!(parse_cell(no, &c).is_err());
            assert!(parse_cell("", &c).is_err());
            assert!(parse_cell("1.0", &c).is_err());
        }
        assert_eq!(
            parse_cell(
                "9007199254740993.123",
                &column("n", "numeric(30,3)", false, false)
            )
            .unwrap(),
            CellValue::Number("9007199254740993.123".into())
        );
        assert!(parse_cell("1e3", &column("n", "numeric", false, false)).is_err());
        assert_eq!(
            parse_cell("NULL", &column("s", "varchar(255)", true, false)).unwrap(),
            CellValue::Text("NULL".into())
        );
        assert!(parse_cell("TRUE", &column("b", "boolean", false, false)).is_err());
        for ty in ["json", "float", "timestamp", "bytea", "date"] {
            assert!(column_kind(&column("x", ty, true, false)).is_err());
        }
    }
    #[test]
    fn malformed_pages_fail_closed() {
        let mut p = (*page()).clone();
        p.truncated = true;
        assert!(can_edit(&p).is_err());
        p.truncated = false;
        p.rows[0].pop();
        assert!(can_edit(&p).is_err());
        let mut p = (*page()).clone();
        p.columns[0].is_primary_key = false;
        assert!(can_edit(&p).is_err());
        let mut p = (*page()).clone();
        p.rows[0][0] = CellValue::Null;
        assert!(can_edit(&p).is_err());
        let mut p = (*page()).clone();
        p.rows[0][1] = CellValue::Binary("00".into());
        assert!(can_edit(&p).is_err());
    }
    #[test]
    fn batch_and_byte_limits_are_atomic() {
        let mut e = edits();
        for _ in 0..100 {
            e.add_row().unwrap();
        }
        assert!(e.add_row().is_err());
        assert_eq!(e.operation_count(), 100);
        e.discard();
        for _ in 0..32 {
            let i = e.add_row().unwrap();
            e.set_cell(i, 1, CellValue::Text("x".repeat(65536)))
                .unwrap();
        }
        let i = e.add_row().unwrap();
        assert!(e.set_cell(i, 1, CellValue::Text("x".into())).is_err());
        assert_eq!(e.displayed(i, 1), None);
        assert!(
            e.set_cell(i, 1, CellValue::Text("x".repeat(65537)))
                .is_err()
        );
        assert!(e.set_cell(i, 1, CellValue::Text("\0".into())).is_err());
    }
}
