use std::borrow::Cow;

use gpui::{AssetSource, IntoElement, SharedString, prelude::*, px, rgb, svg};

#[derive(Clone, Copy)]
pub enum Icon {
    Database,
    Layout,
    Chevron,
    Hide,
    Ai,
    SortAscending,
    SortDescending,
    Download,
}

impl Icon {
    fn path(self) -> &'static str {
        match self {
            Self::Database => "icons/database.svg",
            Self::Layout => "icons/panel-left.svg",
            Self::Chevron => "icons/chevron-down.svg",
            Self::Hide => "icons/minus.svg",
            Self::Ai => "icons/bot-message-square.svg",
            Self::SortAscending => "icons/arrow-up.svg",
            Self::SortDescending => "icons/arrow-down.svg",
            Self::Download => "icons/download.svg",
        }
    }
}

// Embedded assets work independently of the process's working directory and
// require neither a runtime download nor an external icon/font dependency.
const ASSETS: &[(&str, &[u8])] = &[
    (
        "icons/database.svg",
        include_bytes!("../../assets/icons/database.svg"),
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
    fn embedded_icons_are_listed_and_loaded() {
        let assets = IconAssets;
        let paths = assets.list("icons/").unwrap();
        assert_eq!(paths.len(), 8);
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
