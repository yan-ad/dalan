//! A cached explorer projection. Flattening never performs I/O or changes expansion.
use dalan_drivers::{SourceProfile, TableInfo};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TreeKey {
    Source(String),
    Database {
        source: String,
        database: String,
    },
    Group {
        source: String,
        database: String,
        views: bool,
    },
    Table {
        source: String,
        database: String,
        table: String,
        view: bool,
    },
}
impl TreeKey {
    pub fn source(&self) -> &str {
        match self {
            Self::Source(source)
            | Self::Database { source, .. }
            | Self::Group { source, .. }
            | Self::Table { source, .. } => source,
        }
    }
}
#[derive(Clone, Debug)]
pub struct TreeRow {
    pub key: TreeKey,
    pub label: String,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
    pub count: Option<usize>,
    pub status: Option<String>,
}
#[derive(Default, Clone)]
pub struct ExplorerTree {
    pub expanded_sources: HashSet<String>,
    pub expanded_databases: HashSet<(String, String)>,
    /// false = Tables, true = Views.
    pub expanded_groups: HashSet<(String, String, bool)>,
    pub databases: HashMap<String, Vec<String>>,
    pub tables: HashMap<(String, String), Vec<TableInfo>>,
    pub loading: HashSet<TreeKey>,
    pub errors: HashMap<TreeKey, String>,
}
fn label(value: &str) -> String {
    if value.is_empty() {
        "(unnamed)".into()
    } else {
        value.into()
    }
}
/// Schema selection affects display only. Keep the complete discovered catalog cached.
fn visible_database(profile: &SourceProfile, database: &str) -> bool {
    match &profile.schemas {
        dalan_drivers::SchemaSelection::All => true,
        dalan_drivers::SchemaSelection::Selected(names) => {
            names.iter().any(|name| name == database)
        }
    }
}
impl ExplorerTree {
    pub fn has_visible_expansion(&self, profiles: &[SourceProfile]) -> bool {
        profiles
            .iter()
            .any(|profile| self.expanded_sources.contains(&profile.id))
    }

    pub fn collapse_all(&mut self) {
        self.expanded_sources.clear();
        self.expanded_databases.clear();
        self.expanded_groups.clear();
    }
    /// Expand only materialized metadata; never invent children or request metadata.
    pub fn expand_loaded(&mut self) {
        self.expanded_sources.extend(self.databases.keys().cloned());
        for (source, database) in self.tables.keys() {
            if self
                .databases
                .get(source)
                .is_some_and(|dbs| dbs.contains(database))
            {
                self.expanded_sources.insert(source.clone());
                self.expanded_databases
                    .insert((source.clone(), database.clone()));
                for views in [false, true] {
                    self.expanded_groups
                        .insert((source.clone(), database.clone(), views));
                }
            }
        }
    }
    pub fn remove_source(&mut self, source: &str) {
        self.expanded_sources.remove(source);
        self.expanded_databases.retain(|(s, _)| s != source);
        self.expanded_groups.retain(|(s, _, _)| s != source);
        self.databases.remove(source);
        self.tables.retain(|(s, _), _| s != source);
        self.loading.retain(|key| key.source() != source);
        self.errors.retain(|key, _| key.source() != source);
    }
    pub fn retain_profiles(&mut self, profiles: &[SourceProfile]) {
        let valid: HashSet<_> = profiles.iter().map(|p| p.id.as_str()).collect();
        let mut stale: HashSet<String> = self.databases.keys().cloned().collect();
        stale.extend(self.tables.keys().map(|(s, _)| s.clone()));
        stale.extend(self.expanded_sources.iter().cloned());
        stale.extend(self.expanded_databases.iter().map(|(s, _)| s.clone()));
        stale.extend(self.expanded_groups.iter().map(|(s, _, _)| s.clone()));
        stale.extend(
            self.loading
                .iter()
                .chain(self.errors.keys())
                .map(|k| k.source().to_owned()),
        );
        for source in stale {
            if !valid.contains(source.as_str()) {
                self.remove_source(&source);
            }
        }
    }
    fn status(&self, key: &TreeKey) -> Option<String> {
        if self.loading.contains(key) {
            Some("Loading…".into())
        } else {
            self.errors.get(key).cloned()
        }
    }
    pub fn flatten(&self, profiles: &[SourceProfile]) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        for profile in profiles.iter().take(100) {
            if !seen.insert(&profile.id) {
                continue;
            }
            let source = &profile.id;
            let key = TreeKey::Source(source.clone());
            let expanded = self.expanded_sources.contains(source);
            rows.push(TreeRow {
                status: self.status(&key),
                key,
                label: label(&profile.name),
                depth: 0,
                expandable: true,
                expanded,
                count: self.databases.get(source).map(|databases| {
                    databases
                        .iter()
                        .filter(|database| visible_database(profile, database))
                        .count()
                }),
            });
            if !expanded {
                continue;
            }
            let Some(databases) = self.databases.get(source) else {
                continue;
            };
            for database in databases
                .iter()
                .filter(|database| visible_database(profile, database))
                .take(1000)
            {
                let pair = (source.clone(), database.clone());
                let key = TreeKey::Database {
                    source: source.clone(),
                    database: database.clone(),
                };
                let expanded = self.expanded_databases.contains(&pair);
                let tables = self.tables.get(&pair);
                rows.push(TreeRow {
                    status: self.status(&key),
                    key,
                    label: label(database),
                    depth: 1,
                    expandable: true,
                    expanded,
                    count: tables.map(Vec::len),
                });
                if !expanded {
                    continue;
                }
                for views in [false, true] {
                    let key = TreeKey::Group {
                        source: source.clone(),
                        database: database.clone(),
                        views,
                    };
                    let expanded =
                        self.expanded_groups
                            .contains(&(source.clone(), database.clone(), views));
                    let count = tables.map(|items| {
                        items
                            .iter()
                            .filter(|t| (t.kind != "BASE TABLE") == views)
                            .count()
                    });
                    rows.push(TreeRow {
                        status: self.status(&key),
                        key,
                        label: if views { "Views" } else { "Tables" }.into(),
                        depth: 2,
                        expandable: true,
                        expanded,
                        count,
                    });
                    if !expanded {
                        continue;
                    }
                    if let Some(tables) = tables {
                        for table in tables
                            .iter()
                            .take(1000)
                            .filter(|t| (t.kind != "BASE TABLE") == views)
                        {
                            let key = TreeKey::Table {
                                source: source.clone(),
                                database: database.clone(),
                                table: table.name.clone(),
                                view: views,
                            };
                            rows.push(TreeRow {
                                status: self.status(&key),
                                key,
                                label: label(&table.name),
                                depth: 3,
                                expandable: false,
                                expanded: false,
                                count: None,
                            });
                        }
                    }
                }
            }
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile(id: &str) -> SourceProfile {
        SourceProfile {
            id: id.into(),
            name: "quoted `名字' ASCII".into(),
            ..Default::default()
        }
    }
    fn table(name: &str, kind: &str) -> TableInfo {
        TableInfo {
            name: name.into(),
            kind: kind.into(),
        }
    }
    #[test]
    fn schema_selection_filters_projection_without_discarding_metadata() {
        let mut profile = profile("a");
        let mut tree = ExplorerTree::default();
        tree.expanded_sources.insert("a".into());
        tree.databases
            .insert("a".into(), vec!["visible".into(), "hidden".into()]);
        profile.schemas = dalan_drivers::SchemaSelection::Selected(vec!["visible".into()]);
        let rows = tree.flatten(std::slice::from_ref(&profile));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].count, Some(1));
        assert_eq!(rows[1].label, "visible");
        assert_eq!(tree.databases["a"].len(), 2);
        profile.schemas = dalan_drivers::SchemaSelection::Selected(vec![]);
        assert_eq!(tree.flatten(std::slice::from_ref(&profile)).len(), 1);
        profile.schemas = dalan_drivers::SchemaSelection::All;
        assert_eq!(tree.flatten(&[profile]).len(), 3);
    }
    #[test]
    fn compact_lazy_projection_preserves_cache() {
        let profiles = [profile("a")];
        let mut tree = ExplorerTree::default();
        tree.databases
            .insert("a".into(), (0..1000).map(|i| format!("db{i}")).collect());
        tree.tables.insert(
            ("a".into(), "db0".into()),
            (0..1000)
                .map(|i| table(&format!("t{i}"), "BASE TABLE"))
                .collect(),
        );
        tree.expanded_sources.insert("a".into());
        assert_eq!(tree.flatten(&profiles).len(), 1001);
        let mut many_profiles = vec![profile("a")];
        many_profiles.extend((0..99).map(|i| profile(&format!("other{i}"))));
        assert!(tree.flatten(&many_profiles).len() <= 1103);

        tree.expanded_databases.insert(("a".into(), "db0".into()));
        assert_eq!(tree.flatten(&profiles).len(), 1003);
        tree.expanded_groups
            .insert(("a".into(), "db0".into(), false));
        assert_eq!(tree.flatten(&profiles).len(), 2003);
        tree.collapse_all();
        assert_eq!(tree.flatten(&profiles).len(), 1);
        assert_eq!(tree.tables.len(), 1);
        tree.expand_loaded();
        assert_eq!(tree.expanded_databases.len(), 1);
        assert!(tree.loading.is_empty());
    }
    #[test]
    fn isolated_branches_and_unknown_kinds_are_views() {
        let profiles = [profile("a"), profile("b")];
        let mut tree = ExplorerTree::default();
        for source in ["a", "b"] {
            tree.databases
                .insert(source.into(), vec!["one".into(), "two".into()]);
            for database in ["one", "two"] {
                tree.tables.insert(
                    (source.into(), database.into()),
                    vec![
                        table("`漢字'", "BASE TABLE"),
                        table("v", "VIEW"),
                        table("other", "MATERIALIZED VIEW"),
                    ],
                );
            }
        }
        tree.expand_loaded();
        let rows = tree.flatten(&profiles);
        assert_eq!(rows.len(), 26);
        assert!(rows.iter().all(|row| !row.label.is_empty()));
        assert_eq!(
            rows.iter()
                .filter(|r| r.label == "Views" && r.count == Some(2))
                .count(),
            4
        );
        tree.remove_source("a");
        assert_eq!(tree.tables.len(), 2);
        assert_eq!(tree.flatten(&profiles).len(), 14);
        tree.retain_profiles(&[]);
        assert!(tree.databases.is_empty());
    }
    #[test]
    fn errors_are_inline_and_empty_cache_is_not_fabricated() {
        let profiles = [profile("a")];
        let mut tree = ExplorerTree::default();
        tree.expanded_sources.insert("a".into());
        tree.errors
            .insert(TreeKey::Source("a".into()), "Unavailable".into());
        let rows = tree.flatten(&profiles);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].count, None);
        assert_eq!(rows[0].status.as_deref(), Some("Unavailable"));
        tree.databases.insert("a".into(), vec!["".into()]);
        let key = TreeKey::Database {
            source: "a".into(),
            database: "".into(),
        };
        tree.loading.insert(key);
        let rows = tree.flatten(&profiles);
        assert_eq!(rows.len(), 2);
        assert!(!rows[1].label.is_empty());
        assert_eq!(rows[1].status.as_deref(), Some("Loading…"));
    }

    #[test]
    fn hidden_descendant_flags_do_not_count_as_visible_expansion() {
        let profiles = [profile("a")];
        let mut tree = ExplorerTree::default();
        tree.databases.insert("a".into(), vec!["db".into()]);
        tree.tables.insert(
            ("a".into(), "db".into()),
            vec![table("items", "BASE TABLE")],
        );
        tree.expand_loaded();
        assert!(tree.has_visible_expansion(&profiles));
        tree.expanded_sources.remove("a");
        assert!(!tree.expanded_databases.is_empty());
        assert!(!tree.has_visible_expansion(&profiles));
        tree.expanded_sources.insert("removed-source".into());
        assert!(!tree.has_visible_expansion(&profiles));
        tree.expand_loaded();
        assert!(tree.has_visible_expansion(&profiles));
    }
}
