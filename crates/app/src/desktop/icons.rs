use std::borrow::Cow;

use gpui::assets::IconName as KitIconName;
use gpui::{AssetSource, Hsla, IntoElement, SharedString, prelude::*, px};

/// Semantic names for the remaining explorer and connection actions. The glyphs
/// and SVG bytes are owned by GPUI Kit, not by the application.
#[derive(Clone, Copy)]
pub enum Icon {
    Database,
    MariaDb,
    Add,
    Manage,
    Refresh,
    Remove,
    Chevron,
    ChevronRight,
    Folder,
    Table,
    ExpandTree,
    CollapseTree,
    Hide,
    Download,
    Check,
    Cached,
    Warning,
    Loading,
    ReadOnly,
    Previous,
    Query,
    Play,
    Stop,
    Close,
    Filter,
    Sort,
}

impl Icon {
    pub fn kit_name(self) -> KitIconName {
        match self {
            Self::Database => KitIconName::Database,
            Self::MariaDb => KitIconName::DatabaseZap,
            Self::Add => KitIconName::Plus,
            Self::Manage => KitIconName::Settings2,
            Self::Refresh => KitIconName::RefreshCw,
            Self::Remove => KitIconName::Trash,
            Self::Chevron => KitIconName::ChevronDown,
            Self::ChevronRight => KitIconName::ChevronRight,
            Self::Folder => KitIconName::Folder,
            Self::Table => KitIconName::Table,
            Self::ExpandTree => KitIconName::ListTree,
            Self::CollapseTree => KitIconName::ChevronsDownUp,
            Self::Hide => KitIconName::Minus,
            Self::Download => KitIconName::Download,
            Self::Check => KitIconName::Check,
            Self::Cached => KitIconName::HardDrive,
            Self::Warning => KitIconName::TriangleAlert,
            Self::Loading => KitIconName::LoaderCircle,
            Self::ReadOnly => KitIconName::LockKeyhole,
            Self::Previous => KitIconName::ChevronLeft,
            Self::Query => KitIconName::SquareCode,
            Self::Play => KitIconName::Play,
            Self::Stop => KitIconName::CircleStop,
            Self::Close => KitIconName::X,
            Self::Filter => KitIconName::ListFilter,
            Self::Sort => KitIconName::ArrowDownUp,
        }
    }
}

// Extend Kit's curated default component bundle only for the application glyphs
// outside that bundle. This also serves controls using Kit IconName directly.
gpui::assets::icon_assets!(
    AppIconAssets,
    [
        Database,
        DatabaseZap,
        Trash,
        Table,
        ListTree,
        ChevronsDownUp,
        Download,
        LockKeyhole,
        SquareCode,
        CircleStop,
        X,
        ListFilter,
        ArrowDownUp,
        BotMessageSquare,
        KeyRound,
        Hash,
        TextInitial,
        CalendarClock,
        Binary,
        Braces,
    ]
);

const PROVIDER_ASSETS: &[(&str, &[u8])] = &[
    (
        "icons/provider/mysql.svg",
        include_bytes!("../../assets/icons/provider/mysql.svg"),
    ),
    (
        "icons/provider/mariadb.svg",
        include_bytes!("../../assets/icons/provider/mariadb.svg"),
    ),
    (
        "icons/provider/pg.svg",
        include_bytes!("../../assets/icons/provider/pg.svg"),
    ),
    (
        "icons/provider/mongodb.svg",
        include_bytes!("../../assets/icons/provider/mongodb.svg"),
    ),
    (
        "icons/provider/redis.svg",
        include_bytes!("../../assets/icons/provider/redis.svg"),
    ),
    (
        "icons/provider/sqlite.svg",
        include_bytes!("../../assets/icons/provider/sqlite.svg"),
    ),
    (
        "icons/provider/supabase.svg",
        include_bytes!("../../assets/icons/provider/supabase.svg"),
    ),
    (
        "icons/provider/elastic.svg",
        include_bytes!("../../assets/icons/provider/elastic.svg"),
    ),
];

pub(super) struct IconAssets;

impl AssetSource for IconAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = PROVIDER_ASSETS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        if path == "lucide-LICENSE.txt" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/lucide-LICENSE.txt"
            ))));
        }
        // Compatibility for existing controls while migrating them to IconName.
        // Kit uses the canonical Trash filename, not Lucide's deprecated alias.
        let path = if path == "icons/trash-2.svg" {
            "icons/trash.svg"
        } else {
            path
        };
        if let Some(bytes) = AppIconAssets.load(path)? {
            return Ok(Some(bytes));
        }
        // IconName includes the full catalog, while Kit's default Assets is a
        // small subset. Resolve named controls without enumerating that catalog
        // on every SVG load, and fall back to the bundled complete inventory.
        if let Ok(Some(bytes)) = gpui::assets::Assets.load(path) {
            return Ok(Some(bytes));
        }
        if let Ok(Some(bytes)) = gpui::assets::AllAssets.load(path) {
            return Ok(Some(bytes));
        }
        Ok(None)
    }

    fn list(&self, prefix: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths = gpui::assets::AllAssets.list(prefix)?;
        paths.extend(
            PROVIDER_ASSETS
                .iter()
                .filter(|(path, _)| path.starts_with(prefix))
                .map(|(path, _)| SharedString::from(*path)),
        );
        paths.extend(AppIconAssets.list(prefix)?);
        for path in ["lucide-LICENSE.txt", "icons/trash-2.svg"] {
            if path.starts_with(prefix) {
                paths.push(path.into());
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

pub fn provider_icon(engine: dalan_drivers::DbEngine) -> impl IntoElement {
    // SVG masks discard the original fills. Use the color image decoder and
    // stable image handles so the supplied brand artwork stays unchanged.
    static IMAGES: std::sync::OnceLock<Vec<std::sync::Arc<gpui::Image>>> =
        std::sync::OnceLock::new();
    let images = IMAGES.get_or_init(|| {
        [
            "icons/provider/mysql.svg",
            "icons/provider/mariadb.svg",
            "icons/provider/pg.svg",
            "icons/provider/mongodb.svg",
            "icons/provider/redis.svg",
        ]
        .into_iter()
        .map(|path| {
            let bytes = PROVIDER_ASSETS
                .iter()
                .find(|(name, _)| *name == path)
                .expect("bundled provider asset")
                .1;
            std::sync::Arc::new(gpui::Image::from_bytes(
                gpui::ImageFormat::Svg,
                bytes.to_vec(),
            ))
        })
        .collect()
    });
    let index = match engine {
        dalan_drivers::DbEngine::MySql => 0,
        dalan_drivers::DbEngine::MariaDb => 1,
        dalan_drivers::DbEngine::PostgreSql => 2,
        dalan_drivers::DbEngine::MongoDb => 3,
        dalan_drivers::DbEngine::Redis => 4,
    };
    gpui::img(images[index].clone())
        .size(px(16.))
        .flex_shrink_0()
}

/// Kit renders tinted SVG masks with the requested semantic theme color.
pub fn icon(kind: Icon, color: impl Into<Hsla>) -> impl IntoElement {
    gpui::component::Icon::new(kind.kit_name())
        .size(px(16.0))
        .flex_shrink_0()
        .text_color(color.into())
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;

    #[test]
    fn all_registered_kit_icons_load_without_local_svg_copies() {
        let paths = IconAssets.list("icons/").unwrap();
        assert!(!paths.is_empty());
        let mut unique = paths.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(paths, unique);
        for path in paths {
            let bytes = IconAssets.load(path.as_ref()).unwrap().unwrap();
            let svg = std::str::from_utf8(&bytes).unwrap();
            assert!(svg.contains("<svg"), "{path}");
            assert!(!svg.contains("<script"), "{path}");
        }
        for path in gpui::assets::Assets.list("icons/").unwrap() {
            assert_eq!(
                IconAssets.load(&path).unwrap().unwrap(),
                gpui::assets::Assets.load(&path).unwrap().unwrap()
            );
        }
        assert!(IconAssets.list("missing/").unwrap().is_empty());
        assert!(IconAssets.load("icons/missing.svg").unwrap().is_none());
        assert!(IconAssets.load("../icons/database.svg").unwrap().is_none());
    }

    #[test]
    fn query_toolbar_icons_resolve_even_outside_kit_default_subset() {
        for icon in [
            gpui::assets::IconName::TextAlignStart,
            gpui::assets::IconName::Minimize,
            gpui::assets::IconName::Type,
            gpui::assets::IconName::TextWrap,
            gpui::assets::IconName::Save,
            gpui::assets::IconName::FolderOpen,
            gpui::assets::IconName::Clipboard,
            gpui::assets::IconName::Ellipsis,
            gpui::assets::IconName::Sun,
            gpui::assets::IconName::Moon,
        ] {
            assert!(IconAssets.load(&icon.path()).unwrap().is_some(), "{icon:?}");
        }
    }
    #[test]
    fn provider_image_path_preserves_original_multicolor_svg_fills() {
        for (path, color) in [
            ("icons/provider/mysql.svg", "#00618a"),
            ("icons/provider/mariadb.svg", "#003545"),
            ("icons/provider/pg.svg", "#336791"),
            ("icons/provider/mongodb.svg", "#439934"),
            ("icons/provider/redis.svg", "#a41e11"),
        ] {
            let bytes = IconAssets.load(path).unwrap().unwrap();
            assert!(std::str::from_utf8(&bytes).unwrap().contains(color));
        }
        let source = include_str!("icons.rs");
        let implementation = source
            .split("pub fn provider_icon(")
            .nth(1)
            .unwrap()
            .split("/// Kit renders")
            .next()
            .unwrap();
        assert!(
            implementation.contains("ImageFormat::Svg") && implementation.contains("gpui::img(")
        );
        assert!(!implementation.contains("text_color") && !implementation.contains("Icon::empty"));
    }

    #[test]
    fn semantic_actions_and_direct_kit_controls_have_registered_assets() {
        let actions = [
            Icon::Database,
            Icon::MariaDb,
            Icon::Add,
            Icon::Manage,
            Icon::Refresh,
            Icon::Remove,
            Icon::Chevron,
            Icon::ChevronRight,
            Icon::Folder,
            Icon::Table,
            Icon::ExpandTree,
            Icon::CollapseTree,
            Icon::Hide,
            Icon::Download,
            Icon::Check,
            Icon::Cached,
            Icon::Warning,
            Icon::Loading,
            Icon::ReadOnly,
            Icon::Previous,
            Icon::Query,
            Icon::Play,
            Icon::Stop,
            Icon::Close,
            Icon::Filter,
            Icon::Sort,
        ];
        for action in actions {
            assert!(
                IconAssets
                    .load(&action.kit_name().path())
                    .unwrap()
                    .is_some()
            );
        }
        for name in [
            KitIconName::BotMessageSquare,
            KitIconName::KeyRound,
            KitIconName::Hash,
            KitIconName::TextInitial,
            KitIconName::CalendarClock,
            KitIconName::Binary,
            KitIconName::Braces,
        ] {
            assert!(IconAssets.load(&name.path()).unwrap().is_some());
        }
        assert_eq!(Icon::Query.kit_name(), KitIconName::SquareCode);
        assert_ne!(Icon::Database.kit_name(), Icon::MariaDb.kit_name());
        assert_eq!(
            IconAssets.load("icons/trash-2.svg").unwrap(),
            IconAssets.load(&KitIconName::Trash.path()).unwrap()
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
    #[test]
    fn supplied_provider_assets_remain_registered() {
        assert_eq!(IconAssets.list("icons/provider/").unwrap().len(), 8);
        for (path, bytes) in PROVIDER_ASSETS {
            assert_eq!(IconAssets.load(path).unwrap().unwrap().as_ref(), *bytes);
        }
    }
}
