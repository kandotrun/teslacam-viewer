//! Q9: headless GPUI Kit UI test (TestAppContext, no real window server).
use gpui_kit::test::{TestSupportExt, TestWindowExt};
use gpui_kit::{
    AppContext, Bounds, Context, Point, Role, SharedString, TestAppContext, Window, WindowBounds,
    WindowOptions,
    component::button::{Button, ButtonVariants},
    div,
    prelude::*,
    px, size,
};

/// Playback controls with Japanese copy; state lives in the view, logic is a toggle.
struct Controls {
    playing: bool,
}

impl Render for Controls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status: SharedString = if self.playing { "再生中" } else { "一時停止中" }.into();
        div()
            .child(
                Button::new("play-pause")
                    .primary()
                    .label(if self.playing { "一時停止" } else { "再生" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.playing = !this.playing;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("status")
                    .role(Role::Status)
                    .test_support()
                    .aria_label(status.clone())
                    .child(status),
            )
    }
}

#[gpui_kit::test]
fn play_button_toggles_status(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, view) = cx.update(|cx| {
        let bounds = Bounds { origin: Point::default(), size: size(px(640.), px(480.)) };
        gpui_kit::open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            cx,
            |_, cx| cx.new(|_| Controls { playing: false }),
        )
        .expect("open test window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("status").label(), Some("一時停止中"));
        window.click("play-pause", cx);
        assert_eq!(window.find("status").label(), Some("再生中"));
        window.click("play-pause", cx);
        assert_eq!(window.find("status").label(), Some("一時停止中"));
    })
    .unwrap();
    cx.read(|cx| assert!(!view.read(cx).playing));
}
