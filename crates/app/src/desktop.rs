use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};

struct Bootstrap;

impl Render for Bootstrap {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_3()
            .bg(rgb(0x202124))
            .text_color(rgb(0xe8eaed))
            .child("dalan")
            .child(
                "Development bootstrap. Database connections and ACP sessions are not implemented.",
            )
    }
}

pub fn run() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Bootstrap),
        ) {
            eprintln!("Could not open dalan bootstrap window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
