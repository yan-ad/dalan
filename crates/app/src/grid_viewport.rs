//! Fixed-size, two-axis grid geometry, independent of GPUI and page storage.
//! Work is proportional to the viewport, not to the number of loaded cells.
use std::ops::Range;

pub const COLUMN_WIDTH: f32 = 180.;
pub const ROW_HEIGHT: f32 = 22.;
pub const HEADER_HEIGHT: f32 = 28.;
pub const SCROLLBAR_SIZE: f32 = 10.;
pub const OVERSCAN: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GridViewport {
    pub width: f32,
    pub height: f32,
    /// Positive distances from the first column/row.
    pub x: f32,
    pub y: f32,
}

impl GridViewport {
    pub fn max_x(&self, columns: usize) -> f32 {
        (columns as f32 * COLUMN_WIDTH - self.width).max(0.)
    }
    pub fn max_y(&self, rows: usize) -> f32 {
        (rows as f32 * ROW_HEIGHT - self.height).max(0.)
    }
    pub fn clamp(&mut self, rows: usize, columns: usize) {
        self.x = self.x.clamp(0., self.max_x(columns));
        self.y = self.y.clamp(0., self.max_y(rows));
    }
    pub fn resize(&mut self, width: f32, height: f32, rows: usize, columns: usize) {
        self.width = width.max(0.);
        self.height = height.max(0.);
        self.clamp(rows, columns);
    }
    pub fn scroll(&mut self, dx: f32, dy: f32, rows: usize, columns: usize) {
        self.x += dx;
        self.y += dy;
        self.clamp(rows, columns);
    }
    pub fn reset(&mut self) {
        self.x = 0.;
        self.y = 0.;
    }
    /// Exact paint range, without the retained-cache overscan. Partially
    /// visible cells are included; offscreen cells never submit GPU work.
    pub fn painted_columns(&self, count: usize) -> Range<usize> {
        paint_range(self.x, self.width, COLUMN_WIDTH, count)
    }
    pub fn painted_rows(&self, count: usize) -> Range<usize> {
        paint_range(self.y, self.height, ROW_HEIGHT, count)
    }
    pub fn columns(&self, count: usize) -> Range<usize> {
        visible_range(self.x, self.width, COLUMN_WIDTH, count)
    }
    pub fn rows(&self, count: usize) -> Range<usize> {
        visible_range(self.y, self.height, ROW_HEIGHT, count)
    }
}

fn paint_range(offset: f32, extent: f32, cell: f32, count: usize) -> Range<usize> {
    if extent <= 0. || count == 0 {
        return 0..0;
    }
    let first = (offset.max(0.) / cell).floor() as usize;
    let end = ((offset.max(0.) + extent) / cell).ceil() as usize;
    first.min(count)..end.min(count)
}

fn visible_range(offset: f32, extent: f32, cell: f32, count: usize) -> Range<usize> {
    if extent <= 0. || count == 0 {
        return 0..0;
    }
    let first = (offset.max(0.) / cell).floor() as usize;
    let end = ((offset.max(0.) + extent) / cell).ceil() as usize;
    first.saturating_sub(OVERSCAN).min(count)..end.saturating_add(OVERSCAN).min(count)
}

/// Position and length of a thumb within its track. A minimum thumb remains
/// usable even for a 512-column table; the travel still maps to the full extent.
pub fn scrollbar(offset: f32, viewport: f32, content: f32) -> (f32, f32) {
    if viewport <= 0. {
        return (0., 0.);
    }
    if content <= viewport {
        return (0., viewport);
    }
    let length = (viewport * viewport / content).max(20.).min(viewport);
    let position =
        offset.clamp(0., content - viewport) / (content - viewport) * (viewport - length);
    (position, length)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paint_ranges_exclude_cache_overscan() {
        let mut view = GridViewport::default();
        view.resize(720., 220., 200, 512);
        assert_eq!(view.painted_columns(512), 0..4);
        assert_eq!(view.painted_rows(200), 0..10);
        view.scroll(180. * 200. + 1., 22. * 50. + 1., 200, 512);
        assert_eq!(view.painted_columns(512), 200..205);
        assert_eq!(view.painted_rows(200), 50..61);
        assert_eq!(GridViewport::default().painted_rows(200), 0..0);
        assert_eq!(view.painted_columns(0), 0..0);
    }

    #[test]
    fn wide_page_only_visits_visible_cells() {
        let mut view = GridViewport::default();
        view.resize(720., 220., 100, 512);
        assert_eq!(view.columns(512), 0..6);
        assert_eq!(view.rows(100), 0..12);
        view.scroll(180. * 200., 22. * 50., 100, 512);
        assert_eq!(view.columns(512), 198..206);
        assert_eq!(view.rows(100), 48..62);
        assert!(view.columns(512).len() * view.rows(100).len() <= 112);
    }
    #[test]
    fn both_edges_resize_and_reset_are_bounded() {
        let mut view = GridViewport::default();
        view.resize(720., 220., 200, 512);
        view.scroll(f32::MAX, f32::MAX, 200, 512);
        assert_eq!(view.columns(512), 506..512);
        assert_eq!(view.rows(200), 188..200);
        view.resize(180. * 512., 22. * 200., 200, 512);
        assert_eq!((view.x, view.y), (0., 0.));
        view.scroll(-100., -100., 0, 0);
        assert_eq!(view.rows(0), 0..0);
        assert_eq!(view.columns(0), 0..0);
        view.reset();
    }
    #[test]
    fn partial_cells_and_zero_viewports() {
        let view = GridViewport {
            width: 181.,
            height: 23.,
            x: 179.,
            y: 21.,
        };
        assert_eq!(view.columns(512), 0..4);
        assert_eq!(view.rows(100), 0..4);
        assert_eq!(GridViewport::default().columns(512), 0..0);
    }
    #[test]
    fn thumb_reaches_far_edge_of_wide_grid() {
        let (start, length) = scrollbar(512. * 180. - 720., 720., 512. * 180.);
        assert_eq!(length, 20.);
        assert!((start + length - 720.).abs() < 0.01);
        assert_eq!(scrollbar(0., 720., 180.), (0., 720.));
    }
    #[test]
    fn virtualization_bound_holds_at_every_column_and_row_boundary() {
        let mut view = GridViewport::default();
        view.resize(719., 219., 200, 512);
        for column in 0..512 {
            for fraction in [0., 0.5, 0.99] {
                view.x = (column as f32 + fraction) * COLUMN_WIDTH;
                view.y = (column.min(199) as f32 + fraction) * ROW_HEIGHT;
                view.clamp(200, 512);
                let columns = view.columns(512);
                let rows = view.rows(200);
                assert!(columns.end <= 512 && columns.len() <= 9);
                assert!(rows.end <= 200 && rows.len() <= 15);
                assert!(columns.contains(&((view.x / COLUMN_WIDTH).floor() as usize)));
                assert!(rows.contains(&((view.y / ROW_HEIGHT).floor() as usize)));
            }
        }
    }

    #[test]
    fn shrinking_page_clamps_offsets_and_empty_page_has_no_cells() {
        let mut view = GridViewport::default();
        view.resize(720., 220., 200, 512);
        view.scroll(f32::MAX, f32::MAX, 200, 512);
        view.clamp(3, 2);
        assert_eq!((view.x, view.y), (0., 0.));
        assert_eq!(view.columns(2), 0..2);
        assert_eq!(view.rows(3), 0..3);
        view.resize(-1., -1., 0, 0);
        assert_eq!(view, GridViewport::default());
    }

    #[test]
    fn scrollbar_is_bounded_monotonic_and_handles_empty_tracks() {
        for viewport in [0., 10., 20., 720.] {
            let content = 512. * COLUMN_WIDTH;
            let mut previous = 0.;
            for fraction in [-1., 0., 0.25, 0.5, 0.75, 1., 2.] {
                let (position, length) =
                    scrollbar(fraction * (content - viewport), viewport, content);
                assert!(position >= previous);
                assert!(position >= 0. && length >= 0.);
                assert!(position + length <= viewport + 0.001);
                previous = position;
            }
        }
        assert_eq!(scrollbar(100., 0., 1000.), (0., 0.));
        assert_eq!(scrollbar(100., 720., 0.), (0., 720.));
    }
}
