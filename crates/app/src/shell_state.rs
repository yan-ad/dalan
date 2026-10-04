pub const PANE_GAP: f32 = 4.0;
pub const OUTER_PADDING: f32 = 0.0;
pub const MIN_CONTENT_WIDTH: f32 = 240.0;
pub const MIN_SIDEBAR_WIDTH: f32 = 200.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 480.0;
pub const ACP_PANEL_WIDTH: f32 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellLayout {
    pub database: Option<f32>,
    pub acp: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    LayoutMenu,
    ToggleDatabase,
    NarrowDatabase,
    WidenDatabase,
    ResetLayout,
    ToggleAcp,
}

impl Control {
    pub const fn id(self) -> &'static str {
        match self {
            Self::LayoutMenu => "layout-menu",
            Self::ToggleDatabase => "toggle-database",
            Self::NarrowDatabase => "narrow-database",
            Self::WidenDatabase => "widen-database",
            Self::ResetLayout => "reset-layout",
            Self::ToggleAcp => "acp-toggle",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellState {
    pub database_visible: bool,
    pub menu_open: bool,
    pub acp_visible: bool,
    database_width: f32,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            database_visible: true,
            menu_open: false,
            acp_visible: false,
            database_width: 320.0,
        }
    }
}

impl ShellState {
    pub fn apply(&mut self, control: Control) {
        match control {
            Control::LayoutMenu => self.menu_open = !self.menu_open,
            Control::ToggleDatabase => self.database_visible = !self.database_visible,
            Control::NarrowDatabase => self.resize(self.database_width - 32.0),
            Control::WidenDatabase => self.resize(self.database_width + 32.0),
            Control::ResetLayout => *self = Self::default(),
            Control::ToggleAcp => self.acp_visible = !self.acp_visible,
        }
        if control != Control::LayoutMenu {
            self.menu_open = false;
        }
    }

    pub fn resize(&mut self, width: f32) {
        if width.is_finite() {
            self.database_width = width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        }
    }

    pub fn requested_width(&self) -> f32 {
        self.database_width
    }

    /// Compact layouts clamp the visible width without losing the preference.
    pub fn database_width(&self, viewport_width: f32) -> Option<f32> {
        self.layout(viewport_width).database
    }

    pub fn layout(&self, viewport_width: f32) -> ShellLayout {
        if !viewport_width.is_finite() {
            return ShellLayout {
                database: None,
                acp: None,
            };
        }
        let mut budget = viewport_width - 2.0 * OUTER_PADDING - MIN_CONTENT_WIDTH;
        let acp = if self.acp_visible && budget > PANE_GAP {
            let width = ACP_PANEL_WIDTH.min(budget - PANE_GAP);
            budget -= width + PANE_GAP;
            Some(width)
        } else {
            None
        };
        let available = budget - PANE_GAP;
        let database = (self.database_visible && available >= MIN_SIDEBAR_WIDTH)
            .then(|| self.database_width.min(available));
        ShellLayout { database, acp }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_closes_menu_and_restores_visibility() {
        let mut state = ShellState::default();
        state.apply(Control::LayoutMenu);
        assert!(state.menu_open);
        state.apply(Control::ToggleDatabase);
        assert!(!state.database_visible);
        assert!(!state.menu_open);
        assert_eq!(state.database_width(1_200.0), None);
        state.apply(Control::ToggleDatabase);
        assert!(state.database_visible);
    }

    #[test]
    fn resize_clamps_and_ignores_nonfinite_input() {
        let mut state = ShellState::default();
        state.resize(0.0);
        assert_eq!(state.requested_width(), MIN_SIDEBAR_WIDTH);
        state.resize(1_000.0);
        state.resize(f32::NAN);
        state.resize(f32::INFINITY);
        assert_eq!(state.requested_width(), MAX_SIDEBAR_WIDTH);
    }

    #[test]
    fn reset_restores_visibility_and_preferred_width() {
        let mut state = ShellState::default();
        state.apply(Control::ToggleDatabase);
        state.apply(Control::NarrowDatabase);
        state.apply(Control::ResetLayout);
        assert_eq!(state, ShellState::default());
    }

    #[test]
    fn compact_layout_preserves_preference() {
        let mut state = ShellState::default();
        state.resize(480.0);
        assert_eq!(state.database_width(720.0), Some(476.0));
        assert_eq!(state.database_width(1_200.0), Some(480.0));
        assert_eq!(state.requested_width(), 480.0);
        assert_eq!(state.database_width(f32::NAN), None);
        assert_eq!(state.database_width(f32::INFINITY), None);
    }

    #[test]
    fn visible_sidebar_leaves_room_for_content() {
        for viewport in [0.0, 320.0, 540.0, 720.0, 800.0, 900.0, 1_200.0] {
            let state = ShellState::default();
            if let Some(width) = state.database_width(viewport) {
                assert!((MIN_SIDEBAR_WIDTH..=MAX_SIDEBAR_WIDTH).contains(&width));
                assert!(width + PANE_GAP + 2.0 * OUTER_PADDING + MIN_CONTENT_WIDTH <= viewport);
            }
        }
    }

    #[test]
    fn width_controls_adjust_sidebar_preference() {
        for (control, delta) in [
            (Control::NarrowDatabase, -32.0),
            (Control::WidenDatabase, 32.0),
        ] {
            let mut state = ShellState::default();
            let width = state.requested_width();
            state.apply(control);
            assert_eq!(state.requested_width(), width + delta);
        }
    }
    #[test]
    fn acp_toggle_preserves_database_preferences_and_menu_closes() {
        let mut state = ShellState::default();
        state.resize(480.0);
        state.apply(Control::LayoutMenu);
        state.apply(Control::ToggleAcp);
        assert!(state.acp_visible);
        assert!(!state.menu_open);
        assert_eq!(
            state.layout(720.0),
            ShellLayout {
                database: None,
                acp: Some(300.0)
            }
        );
        assert!(state.database_visible);
        state.apply(Control::ToggleAcp);
        assert_eq!(state.layout(720.0).database, Some(476.0));
        assert_eq!(state.requested_width(), 480.0);
    }

    #[test]
    fn acp_layout_remains_bounded_without_changing_preferences() {
        let mut state = ShellState::default();
        state.apply(Control::ToggleAcp);
        for viewport in [320.0, 480.0, 720.0, 900.0, 1_280.0] {
            let layout = state.layout(viewport);
            let panes = layout.database.unwrap_or_default() + layout.acp.unwrap_or_default();
            let gaps = PANE_GAP
                * (usize::from(layout.database.is_some()) + usize::from(layout.acp.is_some()))
                    as f32;
            assert!(panes + gaps + 2.0 * OUTER_PADDING + MIN_CONTENT_WIDTH <= viewport);
        }
        state.apply(Control::ResetLayout);
        assert!(!state.acp_visible);
    }
}
