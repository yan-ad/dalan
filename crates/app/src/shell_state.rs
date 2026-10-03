pub const RAIL_WIDTH: f32 = 36.0;
pub const PANE_GAP: f32 = 6.0;
pub const MIN_CONTENT_WIDTH: f32 = 240.0;
pub const MIN_SIDEBAR_WIDTH: f32 = 200.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 480.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Database,
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    LayoutMenu,
    ToggleDatabase,
    ToggleFiles,
    NarrowDatabase,
    WidenDatabase,
    NarrowFiles,
    WidenFiles,
    ResetLayout,
}

impl Control {
    pub const fn id(self) -> &'static str {
        match self {
            Self::LayoutMenu => "layout-menu",
            Self::ToggleDatabase => "toggle-database",
            Self::ToggleFiles => "toggle-files",
            Self::NarrowDatabase => "narrow-database",
            Self::WidenDatabase => "widen-database",
            Self::NarrowFiles => "narrow-files",
            Self::WidenFiles => "widen-files",
            Self::ResetLayout => "reset-layout",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneWidths {
    pub database: Option<f32>,
    pub files: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellState {
    pub database_visible: bool,
    pub files_visible: bool,
    pub menu_open: bool,
    database_width: f32,
    files_width: f32,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            database_visible: true,
            files_visible: true,
            menu_open: false,
            database_width: 320.0,
            files_width: 240.0,
        }
    }
}

impl ShellState {
    pub fn apply(&mut self, control: Control) {
        match control {
            Control::LayoutMenu => self.menu_open = !self.menu_open,
            Control::ToggleDatabase => self.database_visible = !self.database_visible,
            Control::ToggleFiles => self.files_visible = !self.files_visible,
            Control::NarrowDatabase => self.resize(Side::Database, self.database_width - 32.0),
            Control::WidenDatabase => self.resize(Side::Database, self.database_width + 32.0),
            Control::NarrowFiles => self.resize(Side::Files, self.files_width - 32.0),
            Control::WidenFiles => self.resize(Side::Files, self.files_width + 32.0),
            Control::ResetLayout => *self = Self::default(),
        }
        if control != Control::LayoutMenu {
            self.menu_open = false;
        }
    }

    pub fn resize(&mut self, side: Side, width: f32) {
        if !width.is_finite() {
            return;
        }
        let width = width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        match side {
            Side::Database => self.database_width = width,
            Side::Files => self.files_width = width,
        }
    }

    pub fn requested_width(&self, side: Side) -> f32 {
        match side {
            Side::Database => self.database_width,
            Side::Files => self.files_width,
        }
    }

    /// Compact windows temporarily hide the files pane before the database pane.
    /// User visibility and preferred widths are retained when the window grows.
    pub fn pane_widths(&self, viewport_width: f32) -> PaneWidths {
        let viewport = if viewport_width.is_finite() { viewport_width.max(0.0) } else { 0.0 };
        let budget = (viewport - 2.0 * RAIL_WIDTH - MIN_CONTENT_WIDTH - 2.0 * PANE_GAP).max(0.0);
        let mut database = self.database_visible.then_some(self.database_width);
        let mut files = self.files_visible.then_some(self.files_width);
        if database.is_some() && files.is_some() && budget < 2.0 * MIN_SIDEBAR_WIDTH {
            files = None;
        }
        let count = usize::from(database.is_some()) + usize::from(files.is_some());
        if count == 0 || budget < MIN_SIDEBAR_WIDTH {
            return PaneWidths { database: None, files: None };
        }
        if let (Some(left), Some(right)) = (database, files) {
            let total = left + right;
            if total > budget {
                let extra = budget - 2.0 * MIN_SIDEBAR_WIDTH;
                let desired_extra = total - 2.0 * MIN_SIDEBAR_WIDTH;
                database = Some(MIN_SIDEBAR_WIDTH + extra * (left - MIN_SIDEBAR_WIDTH) / desired_extra);
                files = Some(MIN_SIDEBAR_WIDTH + extra * (right - MIN_SIDEBAR_WIDTH) / desired_extra);
            }
        } else {
            database = database.map(|width| width.min(budget));
            files = files.map(|width| width.min(budget));
        }
        PaneWidths { database, files }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_are_independent_and_close_the_menu() {
        let mut state = ShellState::default();
        state.apply(Control::LayoutMenu);
        assert!(state.menu_open);
        state.apply(Control::ToggleDatabase);
        assert!(!state.database_visible);
        assert!(state.files_visible);
        assert!(!state.menu_open);
        state.apply(Control::ToggleFiles);
        assert!(!state.files_visible);
        state.apply(Control::ToggleDatabase);
        assert!(state.database_visible);
    }

    #[test]
    fn resize_clamps_and_ignores_nonfinite_input() {
        let mut state = ShellState::default();
        state.resize(Side::Database, 0.0);
        state.resize(Side::Files, 1_000.0);
        assert_eq!(state.requested_width(Side::Database), MIN_SIDEBAR_WIDTH);
        assert_eq!(state.requested_width(Side::Files), MAX_SIDEBAR_WIDTH);
        state.resize(Side::Files, f32::NAN);
        state.resize(Side::Files, f32::INFINITY);
        assert_eq!(state.requested_width(Side::Files), MAX_SIDEBAR_WIDTH);
    }

    #[test]
    fn reset_restores_visibility_and_preferred_widths() {
        let mut state = ShellState::default();
        state.apply(Control::ToggleDatabase);
        state.apply(Control::NarrowFiles);
        state.apply(Control::ResetLayout);
        assert_eq!(state, ShellState::default());
    }

    #[test]
    fn compact_layout_preserves_preferences() {
        let state = ShellState::default();
        assert_eq!(state.pane_widths(720.0).files, None);
        assert_eq!(state.pane_widths(720.0).database, Some(320.0));
        assert_eq!(state.pane_widths(1_200.0), PaneWidths { database: Some(320.0), files: Some(240.0) });
        assert!(state.files_visible);
    }

    #[test]
    fn visible_panes_stay_bounded_and_leave_room_for_content() {
        for viewport in [0.0, 320.0, 540.0, 720.0, 800.0, 900.0, 1_200.0] {
            let state = ShellState::default();
            let panes = state.pane_widths(viewport);
            let total = panes.database.unwrap_or_default() + panes.files.unwrap_or_default();
            for width in [panes.database, panes.files].into_iter().flatten() {
                assert!((MIN_SIDEBAR_WIDTH..=MAX_SIDEBAR_WIDTH).contains(&width));
            }
            if total > 0.0 {
                assert!(total + 2.0 * RAIL_WIDTH + 2.0 * PANE_GAP + MIN_CONTENT_WIDTH <= viewport + 0.01);
            }
        }
    }

    #[test]
    fn all_resize_controls_change_the_intended_pane() {
        for (control, side, delta) in [
            (Control::NarrowDatabase, Side::Database, -32.0),
            (Control::WidenDatabase, Side::Database, 32.0),
            (Control::NarrowFiles, Side::Files, -32.0),
            (Control::WidenFiles, Side::Files, 32.0),
        ] {
            let mut state = ShellState::default();
            let width = state.requested_width(side);
            state.apply(control);
            assert_eq!(state.requested_width(side), width + delta);
        }
    }
}
