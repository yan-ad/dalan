use std::borrow::Cow;

use gpui::{AssetSource, IntoElement, SharedString, prelude::*, px, rgb, svg};

#[derive(Clone, Copy)]
pub enum Icon {
    Database,
    MariaDb,
    Add,
    Manage,
    Refresh,
    Remove,
    Layout,
    Chevron,
    ChevronRight,
    Folder,
    Table,
    ExpandTree,
    CollapseTree,
    Hide,
    Ai,
    SortAscending,
    SortDescending,
    Download,
    Check,
    Cached,
    Warning,
    Loading,
    ReadOnly,
    Previous,
}

impl Icon {
    fn path(self) -> &'static str {
        match self {
            Self::Database => "icons/database.svg",
            Self::MariaDb => "icons/database-zap.svg",
            Self::Add => "icons/plus.svg",
            Self::Manage => "icons/settings-2.svg",
            Self::Refresh => "icons/refresh-cw.svg",
            Self::Remove => "icons/trash-2.svg",
            Self::Layout => "icons/panel-left.svg",
            Self::Chevron => "icons/chevron-down.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::Folder => "icons/folder.svg",
            Self::Table => "icons/table.svg",
            Self::ExpandTree => "icons/list-tree.svg",
            Self::CollapseTree => "icons/chevrons-down-up.svg",
            Self::Hide => "icons/minus.svg",
            Self::Ai => "icons/bot-message-square.svg",
            Self::SortAscending => "icons/arrow-up.svg",
            Self::SortDescending => "icons/arrow-down.svg",
            Self::Download => "icons/download.svg",
            Self::Check => "icons/check.svg",
            Self::Cached => "icons/hard-drive.svg",
            Self::Warning => "icons/triangle-alert.svg",
            Self::Loading => "icons/loader-circle.svg",
            Self::ReadOnly => "icons/lock-keyhole.svg",
            Self::Previous => "icons/chevron-left.svg",
        }
    }
}

// Embedded assets work independently of the process's working directory and
// require neither a runtime download nor an external icon/font dependency.
const ASSETS: &[(&str, &[u8])] = &[
    (
        "icons/chevron-right.svg",
        include_bytes!("../../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../../assets/icons/folder.svg"),
    ),
    (
        "icons/table.svg",
        include_bytes!("../../assets/icons/table.svg"),
    ),
    (
        "icons/list-tree.svg",
        include_bytes!("../../assets/icons/list-tree.svg"),
    ),
    (
        "icons/chevrons-down-up.svg",
        include_bytes!("../../assets/icons/chevrons-down-up.svg"),
    ),
    (
        "icons/database.svg",
        include_bytes!("../../assets/icons/database.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../../assets/icons/check.svg"),
    ),
    (
        "icons/panel-left.svg",
        include_bytes!("../../assets/icons/panel-left.svg"),
    ),
    (
        "icons/chevron-down.svg",
        include_bytes!("../../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/minus.svg",
        include_bytes!("../../assets/icons/minus.svg"),
    ),
    (
        "icons/bot-message-square.svg",
        include_bytes!("../../assets/icons/bot-message-square.svg"),
    ),
    (
        "icons/arrow-up.svg",
        include_bytes!("../../assets/icons/arrow-up.svg"),
    ),
    (
        "icons/arrow-down.svg",
        include_bytes!("../../assets/icons/arrow-down.svg"),
    ),
    (
        "icons/download.svg",
        include_bytes!("../../assets/icons/download.svg"),
    ),
    (
        "lucide-LICENSE.txt",
        include_bytes!("../../assets/lucide-LICENSE.txt"),
    ),
    (
        "icons/plus.svg",
        include_bytes!("../../assets/icons/plus.svg"),
    ),
    (
        "icons/settings-2.svg",
        include_bytes!("../../assets/icons/settings-2.svg"),
    ),
    (
        "icons/refresh-cw.svg",
        include_bytes!("../../assets/icons/refresh-cw.svg"),
    ),
    (
        "icons/trash-2.svg",
        include_bytes!("../../assets/icons/trash-2.svg"),
    ),
    (
        "icons/database-zap.svg",
        include_bytes!("../../assets/icons/database-zap.svg"),
    ),
    (
        "icons/hard-drive.svg",
        include_bytes!("../../assets/icons/hard-drive.svg"),
    ),
    (
        "icons/triangle-alert.svg",
        include_bytes!("../../assets/icons/triangle-alert.svg"),
    ),
    (
        "icons/loader-circle.svg",
        include_bytes!("../../assets/icons/loader-circle.svg"),
    ),
    (
        "icons/lock-keyhole.svg",
        include_bytes!("../../assets/icons/lock-keyhole.svg"),
    ),
    (
        "icons/chevron-left.svg",
        include_bytes!("../../assets/icons/chevron-left.svg"),
    ),
];

pub(super) struct IconAssets;

impl AssetSource for IconAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(ASSETS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, prefix: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ASSETS
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

/// GPUI rasterizes SVGs as alpha masks and applies the element's text color.
pub fn icon(kind: Icon, color: u32) -> impl IntoElement {
    svg()
        .path(kind.path())
        .size(px(16.0))
        .flex_shrink_0()
        .text_color(rgb(color))
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;

    #[test]
    fn all_twentyfour_embedded_icons_are_listed_and_loaded() {
        let assets = IconAssets;
        let paths = assets.list("icons/").unwrap();
        assert_eq!(paths.len(), 24);
        for path in paths {
            let bytes = assets.load(path.as_ref()).unwrap().unwrap();
            assert!(matches!(bytes, Cow::Borrowed(_)));
            let svg = std::str::from_utf8(&bytes).unwrap();
            assert!(svg.starts_with("<svg"));
            assert!(svg.contains("viewBox=\"0 0 24 24\""));
            assert!(svg.contains("stroke=\"currentColor\""));
            // The SVG namespace is not an external resource. No glyph may
            // contain scripts, linked resources, or embedded image data.
            for forbidden in [
                "<script",
                "href=",
                "<image",
                "<foreignObject",
                "url(",
                "data:",
                "transform=",
            ] {
                assert!(!svg.contains(forbidden), "{path}: {forbidden}");
            }
        }
        assert_eq!(assets.list("icons/arrow-").unwrap().len(), 2);
        assert_eq!(assets.list(Icon::Ai.path()).unwrap().len(), 1);
        assert!(assets.list("missing/").unwrap().is_empty());
        assert!(assets.load("icons/missing.svg").unwrap().is_none());
        assert!(assets.load("../icons/database.svg").unwrap().is_none());
    }

    #[test]
    fn tree_icons_load_the_expected_assets() {
        for (kind, path, expected) in [
            (
                Icon::Chevron,
                "icons/chevron-down.svg",
                include_bytes!("../../assets/icons/chevron-down.svg").as_slice(),
            ),
            (
                Icon::ChevronRight,
                "icons/chevron-right.svg",
                include_bytes!("../../assets/icons/chevron-right.svg").as_slice(),
            ),
            (
                Icon::Database,
                "icons/database.svg",
                include_bytes!("../../assets/icons/database.svg").as_slice(),
            ),
            (
                Icon::Folder,
                "icons/folder.svg",
                include_bytes!("../../assets/icons/folder.svg").as_slice(),
            ),
            (
                Icon::Table,
                "icons/table.svg",
                include_bytes!("../../assets/icons/table.svg").as_slice(),
            ),
            (
                Icon::ExpandTree,
                "icons/list-tree.svg",
                include_bytes!("../../assets/icons/list-tree.svg").as_slice(),
            ),
            (
                Icon::CollapseTree,
                "icons/chevrons-down-up.svg",
                include_bytes!("../../assets/icons/chevrons-down-up.svg").as_slice(),
            ),
        ] {
            assert_eq!(kind.path(), path);
            assert_eq!(
                IconAssets.load(kind.path()).unwrap().unwrap().as_ref(),
                expected
            );
        }
    }

    #[test]
    fn toolbar_icons_load_the_expected_assets() {
        for (kind, path, expected) in [
            (
                Icon::Add,
                "icons/plus.svg",
                include_bytes!("../../assets/icons/plus.svg").as_slice(),
            ),
            (
                Icon::Manage,
                "icons/settings-2.svg",
                include_bytes!("../../assets/icons/settings-2.svg").as_slice(),
            ),
            (
                Icon::Refresh,
                "icons/refresh-cw.svg",
                include_bytes!("../../assets/icons/refresh-cw.svg").as_slice(),
            ),
            (
                Icon::Remove,
                "icons/trash-2.svg",
                include_bytes!("../../assets/icons/trash-2.svg").as_slice(),
            ),
        ] {
            assert_eq!(kind.path(), path);
            assert_eq!(
                IconAssets.load(kind.path()).unwrap().unwrap().as_ref(),
                expected
            );
        }
    }

    #[test]
    fn status_and_previous_icons_load_the_expected_assets() {
        for (kind, path, expected) in [
            (
                Icon::Cached,
                "icons/hard-drive.svg",
                include_bytes!("../../assets/icons/hard-drive.svg").as_slice(),
            ),
            (
                Icon::Warning,
                "icons/triangle-alert.svg",
                include_bytes!("../../assets/icons/triangle-alert.svg").as_slice(),
            ),
            (
                Icon::Loading,
                "icons/loader-circle.svg",
                include_bytes!("../../assets/icons/loader-circle.svg").as_slice(),
            ),
            (
                Icon::ReadOnly,
                "icons/lock-keyhole.svg",
                include_bytes!("../../assets/icons/lock-keyhole.svg").as_slice(),
            ),
            (
                Icon::Previous,
                "icons/chevron-left.svg",
                include_bytes!("../../assets/icons/chevron-left.svg").as_slice(),
            ),
        ] {
            assert_eq!(kind.path(), path);
            assert_eq!(
                IconAssets.load(kind.path()).unwrap().unwrap().as_ref(),
                expected
            );
        }
    }

    #[test]
    fn engine_icons_have_distinct_paths_and_bytes() {
        assert_eq!(Icon::Database.path(), "icons/database.svg");
        assert_eq!(Icon::MariaDb.path(), "icons/database-zap.svg");
        assert_ne!(Icon::Database.path(), Icon::MariaDb.path());
        let mysql = IconAssets.load(Icon::Database.path()).unwrap().unwrap();
        let mariadb = IconAssets.load(Icon::MariaDb.path()).unwrap().unwrap();
        assert_eq!(
            mysql.as_ref(),
            include_bytes!("../../assets/icons/database.svg")
        );
        assert_eq!(
            mariadb.as_ref(),
            include_bytes!("../../assets/icons/database-zap.svg")
        );
        assert_ne!(mysql.as_ref(), mariadb.as_ref());
    }

    #[test]
    fn ai_uses_conversation_bot_not_decorative_sparkles() {
        assert_eq!(Icon::Ai.path(), "icons/bot-message-square.svg");
        let actual = IconAssets.load(Icon::Ai.path()).unwrap().unwrap();
        assert_eq!(
            actual.as_ref(),
            include_bytes!("../../assets/icons/bot-message-square.svg")
        );
    }

    #[test]
    fn embedded_license_retains_lucide_and_feather_notices() {
        let bytes = IconAssets.load("lucide-LICENSE.txt").unwrap().unwrap();
        let license = std::str::from_utf8(&bytes).unwrap();
        assert!(license.contains("ISC License"));
        assert!(license.contains("Lucide Icons and Contributors"));
        assert!(license.contains("The MIT License (MIT)"));
        assert!(license.contains("Copyright (c) 2013-present Cole Bemis"));
        assert!(license.contains("THE SOFTWARE IS PROVIDED \"AS IS\""));
    }
}
