//! Carbonfox - opaque, adapted from cange/nightfox.zed at the pinned revision
//! recorded in assets/themes/README.md. Both upstream MIT notices are retained.
//! Colors are opaque 24-bit RGB; alpha-bearing upstream fills are composited
//! over PANEL, never passed to GPUI as translucent chrome.

pub const NAME: &str = "Carbonfox - opaque";
pub const BACKGROUND: u32 = 0x161616; // background / editor.background
pub const SURFACE: u32 = 0x0c0c0c; // surface.background
pub const CHROME: u32 = SURFACE; // title_bar.background / status_bar.background
pub const PANEL: u32 = SURFACE; // panel.background
pub const HEADER: u32 = 0x1c1c1c; // toolbar.background (#2a2a2a8c) over PANEL
pub const HOVER: u32 = 0x2a2a2a; // ghost_element.hover (neutral controls)
pub const SELECTION: u32 = 0x242424; // element.selection_background (#52525357) over PANEL
pub const TEXT: u32 = 0xf2f4f8; // editor.foreground
pub const MUTED: u32 = 0xb6b8bb; // terminal.dim_foreground; readable small labels
pub const FOCUS: u32 = 0x78a9ff; // info / terminal.ansi.blue; accessible focus outline
pub const BORDER: u32 = 0x222222; // border (#252525e3) over PANEL; decorative only
pub const INPUT_BG: u32 = BACKGROUND;
pub const INPUT_BORDER: u32 = 0x7b7c7e; // editor.line_number; >=3:1 on control surfaces
pub const TEXT_SELECTION: u32 = SELECTION;
pub const WARNING: u32 = 0xbe95ff; // warning
pub const ERROR: u32 = 0xee5396; // error
pub const SUCCESS: u32 = 0x25be6a; // success

pub const CONTROL_HEIGHT: f32 = 28.0;
pub const CONTROL_RADIUS: f32 = 3.0;
pub const PANE_RADIUS: f32 = 0.0;
pub const TITLEBAR_HEIGHT: f32 = 34.0;
pub const PANEL_HEADER_HEIGHT: f32 = 28.0;
pub const TOOLBAR_HEIGHT: f32 = 28.0;
pub const STATUS_HEIGHT: f32 = 28.0;
pub const GRID_ROW_HEIGHT: f32 = 22.0;

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::*;

    fn luminance(rgb: u32) -> f64 {
        let linear = |shift: u32| {
            let channel = f64::from((rgb >> shift) & 0xff) / 255.0;
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(16) + 0.7152 * linear(8) + 0.0722 * linear(0)
    }

    fn contrast(a: u32, b: u32) -> f64 {
        let a = luminance(a);
        let b = luminance(b);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn opaque_palette_has_readable_text_and_control_boundaries() {
        let surfaces = [
            BACKGROUND, SURFACE, CHROME, PANEL, HEADER, HOVER, SELECTION, INPUT_BG,
        ];
        for background in surfaces {
            for foreground in [TEXT, MUTED] {
                assert!(contrast(foreground, background) >= 4.5);
            }
            for boundary in [FOCUS, INPUT_BORDER] {
                assert!(contrast(boundary, background) >= 3.0);
            }
        }
        // Semantic status labels live on panel/input surfaces, not filled hover controls.
        for background in [PANEL, CHROME, INPUT_BG] {
            for foreground in [WARNING, ERROR, SUCCESS] {
                assert!(contrast(foreground, background) >= 4.5);
            }
        }
        for color in surfaces.into_iter().chain([
            TEXT,
            MUTED,
            FOCUS,
            BORDER,
            INPUT_BORDER,
            TEXT_SELECTION,
            WARNING,
            ERROR,
            SUCCESS,
        ]) {
            assert!(color <= 0x00ff_ffff, "palette must contain only opaque RGB");
        }
    }

    #[test]
    fn palette_matches_the_pinned_carbonfox_opaque_reference() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/themes/carbonfox-opaque.json"))
                .unwrap();
        assert_eq!(reference["name"], NAME);
        assert_eq!(reference["style"]["background.appearance"], "opaque");
        let style = &reference["style"];
        let rgb = |key: &str| {
            let hex = style[key].as_str().unwrap();
            u32::from_str_radix(&hex[1..7], 16).unwrap()
        };
        for (token, key) in [
            (BACKGROUND, "background"),
            (SURFACE, "surface.background"),
            (CHROME, "title_bar.background"),
            (PANEL, "panel.background"),
            (HOVER, "ghost_element.hover"),
            (TEXT, "editor.foreground"),
            (MUTED, "terminal.dim_foreground"),
            (FOCUS, "info"),
            (INPUT_BORDER, "editor.line_number"),
            (WARNING, "warning"),
            (ERROR, "error"),
            (SUCCESS, "success"),
        ] {
            assert_eq!(token, rgb(key), "token mapped from {key}");
        }
        let composite = |key: &str| {
            let hex = style[key].as_str().unwrap();
            let alpha = u32::from_str_radix(&hex[7..9], 16).unwrap();
            [16, 8, 0].into_iter().fold(0, |result, shift| {
                let fg = (rgb(key) >> shift) & 0xff;
                let bg = (PANEL >> shift) & 0xff;
                result | (((fg * alpha + bg * (255 - alpha) + 127) / 255) << shift)
            })
        };
        assert_eq!(HEADER, composite("toolbar.background"));
        assert_eq!(SELECTION, composite("element.selection_background"));
        assert_eq!(BORDER, composite("border"));
        assert_eq!(TEXT_SELECTION, SELECTION);
        assert_eq!(CONTROL_HEIGHT, 28.0);
        assert_eq!(GRID_ROW_HEIGHT, 22.0);
    }
}
