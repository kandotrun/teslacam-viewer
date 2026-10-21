//! Q9: gpui-testing-library 0.1.0 (without its `gpui-kit` feature, which pins
//! gpui-kit =0.7.3) on the same gpui-pre 0.3.9 that gpui-kit 0.8.0 uses.
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, TestAppContext, Window, div, text,
};
use gpui_testing_library::prelude::*;

struct Toggle {
    playing: bool,
}

impl Render for Toggle {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(text!(if self.playing { "再生中" } else { "一時停止中" }).testable())
            .child(
                div()
                    .id("toggle")
                    .role(Role::Button)
                    .aria_label(if self.playing { "一時停止" } else { "再生" })
                    .testable()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.playing = !this.playing;
                        cx.notify();
                    }))
                    .child("▶"),
            )
    }
}

#[gpui_kit::test]
fn toggles_playback_through_user_click(cx: &mut TestAppContext) {
    let screen = render(cx, |_, _| Toggle { playing: false });
    expect(screen.get_by_text("一時停止中")).to_be_visible();
    screen.user().click(&screen.get_by_role(by::role(Role::Button).name("再生")));
    expect(screen.get_by_text("再生中")).to_be_visible();
    expect(screen.get_by_role(by::role(Role::Button).name("一時停止"))).to_be_visible();
}
