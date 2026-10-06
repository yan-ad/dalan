//! Pure workspace identity and navigation. Entities, SQL drafts and network jobs
//! belong to the UI, not this bounded tab registry. UUIDs are session-only IDs.
use anyhow::{Result, bail};
use uuid::Uuid;

pub const MAX_WORKSPACE_TABS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceOpen {
    Table {
        source: String,
        database: String,
        table: String,
    },
    Console {
        source: String,
        database: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TabKey {
    Table {
        source: String,
        database: String,
        table: String,
    },
    Console(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabDescriptor {
    pub id: String,
    pub label: String,
    pub source: String,
    pub database: Option<String>,
    pub kind: WorkspaceOpen,
    pub key: TabKey,
    pub tooltip: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabOpen {
    pub id: String,
    pub created: bool,
}

#[derive(Default)]
pub struct WorkspaceTabs {
    tabs: Vec<TabDescriptor>,
    active: Option<String>,
    console_counter: u64,
}
impl WorkspaceTabs {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn tabs(&self) -> &[TabDescriptor] {
        &self.tabs
    }
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }
    pub fn open_table(&mut self, target: WorkspaceOpen) -> Result<TabOpen> {
        let WorkspaceOpen::Table {
            source,
            database,
            table,
        } = target
        else {
            bail!("Expected a table target")
        };
        let key = TabKey::Table {
            source: source.clone(),
            database: database.clone(),
            table: table.clone(),
        };
        if let Some(tab) = self.tabs.iter().find(|tab| tab.key == key) {
            let id = tab.id.clone();
            self.active = Some(id.clone());
            return Ok(TabOpen { id, created: false });
        }
        self.check_capacity()?;
        let id = Uuid::new_v4().to_string();
        let tooltip = format!("{source} / {database} / {table}");
        self.tabs.push(TabDescriptor {
            id: id.clone(),
            label: table.clone(),
            source: source.clone(),
            database: Some(database.clone()),
            kind: WorkspaceOpen::Table {
                source,
                database,
                table,
            },
            key,
            tooltip,
        });
        self.active = Some(id.clone());
        Ok(TabOpen { id, created: true })
    }
    pub fn open_console(&mut self, source: String, database: Option<String>) -> Result<TabOpen> {
        self.check_capacity()?;
        self.console_counter += 1;
        let id = Uuid::new_v4().to_string();
        let label = format!("Console {}", self.console_counter);
        let tooltip = format!(
            "{source} / {} / {label}",
            database.as_deref().unwrap_or("No database selected")
        );
        self.tabs.push(TabDescriptor {
            id: id.clone(),
            label,
            source: source.clone(),
            database: database.clone(),
            kind: WorkspaceOpen::Console { source, database },
            key: TabKey::Console(id.clone()),
            tooltip,
        });
        self.active = Some(id.clone());
        Ok(TabOpen { id, created: true })
    }
    pub fn update_console_target(&mut self, id: &str, source: String, database: Option<String>) {
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|t| t.id == id && matches!(t.key, TabKey::Console(_)))
        {
            tab.source = source.clone();
            tab.database = database.clone();
            tab.kind = WorkspaceOpen::Console {
                source: source.clone(),
                database: database.clone(),
            };
            tab.tooltip = format!(
                "{source} / {} / {}",
                database.as_deref().unwrap_or("No database selected"),
                tab.label
            );
        }
    }
    fn check_capacity(&self) -> Result<()> {
        if self.tabs.len() >= MAX_WORKSPACE_TABS {
            bail!("At most 32 tabs can be open. Close a tab first.")
        }
        Ok(())
    }
    pub fn activate(&mut self, id: &str) -> bool {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return false;
        }
        self.active = Some(id.to_owned());
        true
    }
    /// Closing the active tab selects its left neighbor (or first remaining tab).
    /// Closing an inactive tab leaves the selection and all other state intact.
    pub fn close(&mut self, id: &str) -> Option<String> {
        let index = self.tabs.iter().position(|tab| tab.id == id)?;
        self.tabs.remove(index);
        if self.active.as_deref() == Some(id) {
            self.active = self
                .tabs
                .get(index.saturating_sub(1))
                .map(|tab| tab.id.clone());
        }
        self.active.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table(source: &str) -> WorkspaceOpen {
        WorkspaceOpen::Table {
            source: source.into(),
            database: "db".into(),
            table: "t".into(),
        }
    }
    #[test]
    fn qualified_identity_and_duplicate_activation() {
        let mut tabs = WorkspaceTabs::new();
        let a = tabs.open_table(table("a")).unwrap();
        let b = tabs.open_table(table("b")).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(
            tabs.open_table(table("a")).unwrap(),
            TabOpen {
                id: a.id.clone(),
                created: false
            }
        );
        assert_eq!(tabs.active(), Some(a.id.as_str()));
        assert_eq!(tabs.tabs().len(), 2);
    }
    #[test]
    fn close_neighbors_and_inactive_preservation() {
        let mut tabs = WorkspaceTabs::new();
        let a = tabs.open_table(table("a")).unwrap();
        let b = tabs.open_table(table("b")).unwrap();
        let c = tabs.open_table(table("c")).unwrap();
        assert_eq!(tabs.close(&c.id), Some(b.id.clone()));
        assert!(tabs.activate(&a.id));
        assert_eq!(tabs.close(&b.id), Some(a.id.clone()));
        assert_eq!(tabs.close(&a.id), None);
        assert!(!tabs.activate("missing"));
    }
    #[test]
    fn capacity_duplicates_and_monotonic_consoles() {
        let mut tabs = WorkspaceTabs::new();
        let a = tabs.open_table(table("a")).unwrap();
        for _ in 1..32 {
            tabs.open_console("a".into(), None).unwrap();
        }
        assert!(tabs.open_console("a".into(), None).is_err());
        assert!(!tabs.open_table(table("a")).unwrap().created);
        let ids: std::collections::HashSet<_> = tabs.tabs().iter().map(|tab| &tab.id).collect();
        assert_eq!(ids.len(), 32);
        tabs.close(&a.id);
        tabs.open_console("a".into(), None).unwrap();
        assert_eq!(tabs.tabs().last().unwrap().label, "Console 32");
    }
}
