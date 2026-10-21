//! A text element that wraps Japanese copy with [`break_lines`] (no one-character tails, no line
//! starting with `、` / `。`) instead of GPUI's break-anywhere wrapping. It uses the inherited text
//! style (one font and colour), like a plain string child.

use gpui_kit::{
    App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, SharedString, Size, Style, TextAlign, TextStyle, Window, px,
    size,
};

use crate::presentation::line_break::break_lines;

/// Wrapped Japanese text (see the module docs).
pub fn ja_text(text: impl Into<SharedString>) -> JaText {
    JaText { text: text.into() }
}

/// See [`ja_text`].
pub struct JaText {
    text: SharedString,
}

fn lines(text: &str, style: &TextStyle, width: Option<Pixels>, window: &Window) -> Vec<String> {
    let font_size = style.font_size.to_pixels(window.rem_size());
    let measure = |line: &str| {
        window
            .text_system()
            .shape_line(
                line.to_owned().into(),
                font_size,
                &[style.to_run(line.len())],
                None,
            )
            .width
            .as_f32()
    };
    match width {
        Some(width) => break_lines(text, width.as_f32(), measure),
        None => text.split('\n').map(str::to_owned).collect(),
    }
}

fn shaped_width(line: &str, style: &TextStyle, window: &Window) -> Pixels {
    window
        .text_system()
        .shape_line(
            line.to_owned().into(),
            style.font_size.to_pixels(window.rem_size()),
            &[style.to_run(line.len())],
            None,
        )
        .width
}

impl IntoElement for JaText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for JaText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let text = self.text.clone();
        let style = window.text_style();
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known: Size<Option<Pixels>>, available: Size<AvailableSpace>, window, _| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    AvailableSpace::MinContent => Some(px(0.0)),
                    AvailableSpace::MaxContent => None,
                });
                let lines = lines(&text, &style, width, window);
                let line_height = style.line_height_in_pixels(window.rem_size());
                let widest = lines
                    .iter()
                    .map(|line| shaped_width(line, &style, window))
                    .fold(px(0.0), |widest, width| widest.max(width));
                size(widest.ceil(), line_height * lines.len() as f32)
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = style.line_height_in_pixels(window.rem_size());
        // A hair of slack so rounding never re-wraps the line that fitted at layout time.
        let width = bounds.size.width + px(0.5);
        for (index, line) in lines(&self.text, &style, Some(width), window)
            .into_iter()
            .enumerate()
        {
            let shaped = window.text_system().shape_line(
                line.clone().into(),
                font_size,
                &[style.to_run(line.len())],
                None,
            );
            let origin = bounds.origin + gpui_kit::point(px(0.0), line_height * index as f32);
            let _ = shaped.paint(origin, line_height, TextAlign::Left, None, window, cx);
        }
    }
}
