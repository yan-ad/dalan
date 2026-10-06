// Fixture patterns adapted from gpui-kit 0.7.1 tests/common and tests/input/editor.rs
// https://github.com/longbridge/gpui-kit — Apache-2.0.
extern crate gpui_kit as gpui;
use dalan_kit_pilot::{COLS, Pilot, ROWS, SQL};
use gpui_kit::component::highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, ClipboardItem, Entity, Point, TestAppContext, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};
use std::sync::atomic::Ordering;

fn fixture(cx: &mut TestAppContext) -> (WindowHandle<gpui_kit::base::Root>, Entity<Pilot>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
    });
    cx.update(|cx| {
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1040.), px(760.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Pilot::new(window, cx)),
        )
        .unwrap();
        (window.downcast::<gpui_kit::base::Root>().unwrap(), view)
    })
}

#[gpui_kit::test]
fn sql_editor_native_typing_indent_undo_paste_focus_caret(cx: &mut TestAppContext) {
    let (handle, view) = fixture(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        let editor = view.read(cx).editor.clone();
        let id = ("input", editor.entity_id());
        window.click(id, cx);
        assert_eq!(window.find(id).focused(), Some(true));
        window.press("secondary-a", cx);
        window.input("SELECT 1;", cx);
        window.press("enter", cx);
        window.input("SELECT 2;", cx);
        assert_eq!(editor.read(cx).value(), "SELECT 1;\nSELECT 2;");
        assert_eq!(editor.read(cx).selected_range(), 19..19);
        window.press("secondary-a", cx);
        window.press("tab", cx);
        assert_eq!(editor.read(cx).value(), "  SELECT 1;\n  SELECT 2;");
        window.press("secondary-z", cx);
        assert_eq!(editor.read(cx).value(), "SELECT 1;\nSELECT 2;");
        cx.write_to_clipboard(ClipboardItem::new_string(SQL.to_owned()));
        window.press("secondary-a", cx);
        window.press("secondary-v", cx);
        assert_eq!(editor.read(cx).value(), SQL);
        assert_eq!(editor.read(cx).selected_range(), SQL.len()..SQL.len());
        // Exercise the actual configured Tree-sitter SQL grammar using the editor's rope.
        let grammar = LanguageRegistry::singleton()
            .language("sql")
            .expect("SQL enabled");
        assert!(grammar.has_grammar());
        let mut syntax = SyntaxHighlighter::new("sql");
        assert!(syntax.update(None, editor.read(cx).text(), None));
        let styles = syntax.styles(&(0..SQL.len()), &*HighlightTheme::default_dark());
        assert!(
            !styles.is_empty(),
            "SQL highlight query must produce actual spans"
        );
        assert!(
            styles
                .iter()
                .any(|(range, _)| range.start == 0 && range.end == 6),
            "SELECT keyword must be highlighted"
        );
        let tree = syntax.tree().expect("real SQL parse tree");
        assert!(
            !tree.root_node().has_error(),
            "{}",
            tree.root_node().to_sexp()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn populated_table_is_bounded_and_measures_horizontal_scroll(cx: &mut TestAppContext) {
    let (handle, view) = fixture(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let pane = window.find("result-pane");
        assert!(pane.visible());
        assert!(pane.bounds().size.width > px(900.));
        assert!(pane.bounds().size.height > px(100.));
        assert!(pane.bounds().size.height < px(520.));
        assert!(window.find("fixture-0-0").visible());
        let calls = view.read(cx).render_calls.clone();
        let first = calls.swap(0, Ordering::Relaxed);
        let table = view.read(cx).table.clone();
        table.update(cx, |table, cx| table.scroll_to_col(COLS - 1, cx));
        window.render_frame(cx);
        let last = calls.swap(0, Ordering::Relaxed);
        assert!(window.find("fixture-0-511").visible());
        eprintln!("KIT_TABLE_BUDGET 1040x760 rows={ROWS} cols={COLS} initial_calls={first} scroll_col_511_calls={last}");
        // Strict enough to reject all 512 columns per visible row; includes overscan and frames.
        assert!(first > 0 && first < 2000, "initial visible-cell budget exceeded: {first}");
        assert!(last > 0 && last < 2000, "horizontal visible-cell budget exceeded: {last}");
        assert_eq!(view.read(cx).fixture.len(), ROWS);
        assert_eq!(view.read(cx).fixture[99].len(), COLS);
    }).unwrap();
}

#[gpui_kit::test]
fn controls_update_the_owner_through_pointer_events(cx: &mut TestAppContext) {
    let (handle, view) = fixture(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("fixture-toggle", cx);
        assert!(view.read(cx).checked);
        window.click("fixture-toggle", cx);
        assert!(!view.read(cx).checked);
        window.click("run-fixture", cx);
        assert_eq!(view.read(cx).clicks, 1);
        // Buttons only count local fixture actions; there is no connection/query executor.
    })
    .unwrap();
}
