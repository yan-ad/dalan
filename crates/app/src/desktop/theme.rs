//! Application layout metrics and semantic colors from the active Kit theme.
//! No application palette is installed: theme selection remains Kit-owned.
use gpui::component::ActiveTheme;
use gpui::{App, Hsla};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Hsla,
    pub chrome: Hsla,
    pub panel: Hsla,
    pub header: Hsla,
    pub hover: Hsla,
    pub selection: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub focus: Hsla,
    pub border: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
    pub success: Hsla,
    pub table: Hsla,
    pub table_even: Hsla,
    pub table_row_border: Hsla,
    pub scrollbar: Hsla,
    pub scrollbar_thumb: Hsla,
}

pub fn colors(cx: &App) -> Palette {
    let theme = cx.theme();
    Palette {
        background: theme.background,
        chrome: theme.title_bar,
        panel: theme.sidebar,
        header: theme.table_head,
        hover: theme.table_hover,
        selection: theme.table_active,
        text: theme.foreground,
        muted: theme.muted_foreground,
        focus: theme.ring,
        border: theme.border,
        warning: theme.warning,
        error: theme.danger,
        success: theme.success,
        table: theme.table,
        table_even: theme.table_even,
        table_row_border: theme.table_row_border,
        scrollbar: theme.scrollbar,
        scrollbar_thumb: theme.scrollbar_thumb,
    }
}

pub const CONTROL_HEIGHT: f32 = 28.0;
pub const CONTROL_RADIUS: f32 = 3.0;
pub const PANE_RADIUS: f32 = 0.0;
pub const TITLEBAR_HEIGHT: f32 = 34.0;
pub const PANEL_HEADER_HEIGHT: f32 = 28.0;
pub const TOOLBAR_HEIGHT: f32 = 28.0;
pub const STATUS_HEIGHT: f32 = 28.0;
pub const GRID_ROW_HEIGHT: f32 = 22.0;
