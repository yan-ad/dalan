//! Best-effort budgets for retained result pages, not total process memory.
//!
//! Active and in-flight pages are protected. Temporary render/export snapshots,
//! schema metadata, drafts, and other UI allocations are outside this estimate.

pub const MAX_RETAINED_RESULT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_RETAINED_RESULT_PAGES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultEntry {
    pub id: String,
    pub bytes: usize,
    pub loaded: bool,
    pub protected: bool,
    pub last_used: u64,
}

/// Evict oldest eligible pages until both estimated limits are met.
/// Protected pages can exceed either limit; in that case all eligible pages
/// are removed and the remaining overage is deliberately left alone.
pub fn eviction_plan(entries: &[ResultEntry], max_bytes: usize, max_pages: usize) -> Vec<String> {
    // A widened total avoids overflow even when individual estimates saturate.
    let mut bytes: u128 = entries
        .iter()
        .filter(|e| e.loaded)
        .map(|e| e.bytes as u128)
        .sum();
    let mut pages = entries.iter().filter(|e| e.loaded).count();
    let mut candidates: Vec<_> = entries
        .iter()
        .filter(|e| e.loaded && !e.protected)
        .collect();
    candidates.sort_by(|a, b| a.last_used.cmp(&b.last_used).then_with(|| a.id.cmp(&b.id)));
    let mut plan = Vec::new();
    for entry in candidates {
        if bytes <= max_bytes as u128 && pages <= max_pages {
            break;
        }
        bytes -= entry.bytes as u128;
        pages -= 1;
        plan.push(entry.id.clone());
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, bytes: usize, last_used: u64, protected: bool) -> ResultEntry {
        ResultEntry {
            id: id.into(),
            bytes,
            loaded: true,
            protected,
            last_used,
        }
    }

    #[test]
    fn page_cap_evicts_oldest_without_touching_active_or_other_protected_pages() {
        let entries = [
            entry("active", 1, 0, true),
            entry("export", 1, 1, true),
            entry("recent", 1, 4, false),
            entry("old", 1, 2, false),
        ];
        assert_eq!(eviction_plan(&entries, 100, 3), ["old"]);
    }

    #[test]
    fn byte_cap_is_independent_of_page_count() {
        let entries = [
            entry("old", 9 * 1024 * 1024, 0, false),
            entry("active", 8 * 1024 * 1024, 1, true),
        ];
        assert_eq!(
            eviction_plan(&entries, MAX_RETAINED_RESULT_BYTES, 8),
            ["old"]
        );
        assert!(eviction_plan(&entries, 17 * 1024 * 1024, 8).is_empty());
    }

    #[test]
    fn protected_overage_is_best_effort_and_unloaded_entries_do_not_count() {
        let mut unloaded = entry("unloaded", usize::MAX, 0, false);
        unloaded.loaded = false;
        let entries = [
            unloaded,
            entry("active", usize::MAX, 0, true),
            entry("busy", usize::MAX, 1, true),
            entry("old", 2, 2, false),
        ];
        assert_eq!(eviction_plan(&entries, 1, 1), ["old"]);
        assert!(eviction_plan(&entries[..3], 1, 1).is_empty());
    }

    #[test]
    fn ties_are_deterministic_and_zero_budget_removes_only_loaded_eligible_pages() {
        let entries = [entry("b", 0, 0, false), entry("a", 0, 0, false)];
        assert_eq!(eviction_plan(&entries, 0, 0), ["a", "b"]);
        assert!(eviction_plan(&[], 0, 0).is_empty());
    }
}
