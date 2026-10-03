use gpui::{IntoElement, PathBuilder, canvas, point, prelude::*, px, rgb};

#[derive(Clone, Copy)]
pub enum Icon {
    Database,
    Files,
    Layout,
    Chevron,
    Hide,
}

// Small utility glyphs are drawn locally, without third-party assets or fonts.
pub fn icon(kind: Icon, color: u32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let mut path = PathBuilder::stroke(px(1.25));
            let p = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
            match kind {
                Icon::Database => {
                    path.move_to(p(3.0, 4.0));
                    path.curve_to(p(13.0, 4.0), p(8.0, 0.5));
                    path.curve_to(p(3.0, 4.0), p(8.0, 7.5));
                    path.line_to(p(3.0, 12.0));
                    path.curve_to(p(13.0, 12.0), p(8.0, 15.5));
                    path.line_to(p(13.0, 4.0));
                    path.move_to(p(3.0, 8.0));
                    path.curve_to(p(13.0, 8.0), p(8.0, 11.5));
                }
                Icon::Files => {
                    path.move_to(p(2.0, 4.0));
                    path.line_to(p(6.0, 4.0));
                    path.line_to(p(7.5, 6.0));
                    path.line_to(p(14.0, 6.0));
                    path.line_to(p(14.0, 13.0));
                    path.line_to(p(2.0, 13.0));
                    path.close();
                }
                Icon::Layout => {
                    path.move_to(p(2.0, 3.0));
                    path.line_to(p(14.0, 3.0));
                    path.line_to(p(14.0, 13.0));
                    path.line_to(p(2.0, 13.0));
                    path.close();
                    path.move_to(p(6.0, 3.0));
                    path.line_to(p(6.0, 13.0));
                    path.move_to(p(11.0, 3.0));
                    path.line_to(p(11.0, 13.0));
                }
                Icon::Chevron => {
                    path.move_to(p(4.0, 6.0));
                    path.line_to(p(8.0, 10.0));
                    path.line_to(p(12.0, 6.0));
                }
                Icon::Hide => {
                    path.move_to(p(4.0, 8.0));
                    path.line_to(p(12.0, 8.0));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color));
            }
        },
    )
    .size(px(16.0))
    .flex_shrink_0()
}
