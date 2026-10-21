//! The floating library (`DESIGN.md` §6 Library; Welcome, scanning, errors): the welcome panel
//! before a folder is loaded, then the categories, the day with its rail, the virtualised event
//! rows and the scan warnings.

use std::cell::Cell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::TestSupportExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, AppContext as _, ClickEvent, Context, Entity,
    FocusHandle, Focusable as _, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Role, ScrollStrategy, StatefulInteractiveElement as _, Styled as _, StyledImage as _,
    Subscription, Toggled, UniformListScrollHandle, Window, anchored, deferred, div, image_cache,
    img, point, px, relative, uniform_list,
};
use teslacam_core::app_state::{AppAction, AppState, Step, Workspace};
use teslacam_core::calendar::{YearMonth, month_grid};
use teslacam_core::camera::ClipCategory;
use teslacam_core::copy::{self, FolderError};
use teslacam_core::event_copy::{EventRowCopy, event_row};
use teslacam_core::filters::CategoryFilter;
use teslacam_core::library::ScanProgress;
use teslacam_core::rail_layout::day_rail as build_day_rail;
use teslacam_core::timestamp::parse_filename_local_timestamp;

use super::components::{
    ChipSize, KeyboardFocus as _, category_chip, category_icon, glass, icon_button, primary_button,
    segment_selected, text_button,
};
use super::date_picker::{PickerAction, date_picker};
use super::rail::{RailClick, day_rail};
use super::style::{AppTheme, Palette, TypeExt as _, icon, numeral_text};
use super::thumbnails::ThumbnailCache;
use crate::assets::Icon;
use crate::store::{AppStore, StoreEvent};

/// Height of one event row including the 2 pt gap below it.
const ROW_STRIDE: f32 = 90.0;

/// The open cluster: its rows replace the day's rows until it is closed or the day changes.
#[derive(Debug, Clone, PartialEq)]
struct OpenCluster {
    day_key: String,
    id: String,
    event_ids: Vec<String>,
    label: String,
}

/// One row of the virtual list.
#[derive(Debug, Clone)]
struct Row {
    id: String,
    copy: EventRowCopy,
    thumbnail: Option<PathBuf>,
    selected: bool,
}

/// The library panel view.
pub struct LibraryPanel {
    store: Entity<AppStore>,
    search: Entity<InputState>,
    search_open: bool,
    picker_month: Option<YearMonth>,
    cluster: Option<OpenCluster>,
    rail_width: Rc<Cell<f32>>,
    /// One tab-stop handle per day rail item (grown as needed).
    rail_focus: Vec<FocusHandle>,
    list: UniformListScrollHandle,
    thumbnails: Entity<ThumbnailCache>,
    rows: Rc<Vec<Row>>,
    _subscriptions: Vec<Subscription>,
}

impl LibraryPanel {
    /// A panel reading `store`.
    pub fn new(
        store: Entity<AppStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> LibraryPanel {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(copy::SEARCH_PLACEHOLDER));
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.subscribe(&store, |this, _, event, cx| match event {
                StoreEvent::LibraryLoaded => {
                    this.cluster = None;
                    this.picker_month = None;
                    this.search_open = false;
                }
                StoreEvent::SessionChanged => this.scroll_to_selection(cx),
            }),
            cx.subscribe(&search, |this, search, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = search.read(cx).value().to_string();
                    this.store.update(cx, |store, cx| {
                        store.dispatch(AppAction::QueryFiltered(query), cx);
                    });
                }
            }),
        ];
        LibraryPanel {
            store,
            search,
            search_open: false,
            picker_month: None,
            cluster: None,
            // The track of a 372 pt panel until the first measurement.
            rail_width: Rc::new(Cell::new(330.0)),
            rail_focus: Vec::new(),
            list: UniformListScrollHandle::new(),
            thumbnails: cx.new(ThumbnailCache::new),
            rows: Rc::new(Vec::new()),
            _subscriptions: subscriptions,
        }
    }

    #[cfg(test)]
    /// The day rail item that has keyboard focus.
    pub fn focused_rail_item(&self, window: &Window) -> Option<usize> {
        self.rail_focus
            .iter()
            .position(|handle| handle.is_focused(window))
    }

    /// The search field's focus handle (shortcuts are ignored while it has focus).
    pub fn search_focus(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }

    /// Opens the search field with `query` (development affordance and ⌘F-like flows).
    pub fn open_search(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        self.search
            .update(cx, |search, cx| search.set_value(query, window, cx));
        // A programmatic value change emits no `InputEvent::Change`.
        self.dispatch(AppAction::QueryFiltered(query.to_owned()), cx);
    }

    /// Whether the date picker is open.
    pub fn has_open_popover(&self) -> bool {
        self.picker_month.is_some()
    }

    /// Closes the date picker; whether it was open.
    pub fn close_popovers(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.picker_month.take().is_some();
        cx.notify();
        open
    }

    /// Opens the date picker on the day on screen.
    pub fn open_date_picker(&mut self, cx: &mut Context<Self>) {
        self.picker_month = self
            .store
            .read(cx)
            .workspace()
            .and_then(Workspace::timeline_date)
            .and_then(|day| YearMonth::of_date_key(&day));
        cx.notify();
    }

    /// Opens the rail cluster that contains `event_id` on the day on screen.
    pub fn open_cluster_of(&mut self, event_id: &str, cx: &mut Context<Self>) {
        let width = f64::from(self.rail_width.get());
        let cluster = self.store.read(cx).workspace().and_then(|workspace| {
            let day = workspace.timeline_date()?;
            let events = workspace.timeline_events();
            build_day_rail(&events, Some(&day), width)
                .items
                .into_iter()
                .find(|item| !item.is_marker() && item.cluster.contains(event_id))
                .map(|item| open_cluster(&day, &item.cluster))
        });
        self.cluster = cluster;
        cx.notify();
    }

    fn scroll_to_selection(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace() else {
            return;
        };
        let selected = workspace.selected_event_id.as_deref();
        if let Some(index) = workspace
            .day_events()
            .iter()
            .position(|event| Some(event.id.as_str()) == selected)
        {
            self.list.scroll_to_item(index, ScrollStrategy::Nearest);
        }
    }

    fn dispatch(&mut self, action: AppAction, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.dispatch(action, cx));
    }
}

fn open_cluster(
    day_key: &str,
    cluster: &teslacam_core::day_timeline::DayTimelineCluster,
) -> OpenCluster {
    OpenCluster {
        day_key: day_key.to_owned(),
        id: cluster.id().to_owned(),
        event_ids: cluster
            .markers
            .iter()
            .map(|marker| marker.event_id.clone())
            .collect(),
        label: copy::cluster_filter_text(
            cluster.start_time_label(),
            cluster.end_time_label(),
            cluster.count(),
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------------------------

impl LibraryPanel {
    fn header(&self, folder_name: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let palette = AppTheme::get(cx).palette;
        let fill_hover = palette.fill_hover;
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .min_h(px(62.0))
            .pt(px(14.0))
            .pr(px(14.0))
            .pb(px(8.0))
            .pl(px(20.0))
            .child(
                div()
                    .mr_auto()
                    .numeral()
                    .text_size(px(15.0))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(palette.ink)
                    .whitespace_nowrap()
                    .child(copy::APP_NAME),
            )
            .when_some(folder_name, |header, folder_name| {
                let glyph = if self.search_open {
                    Icon::X
                } else {
                    Icon::Search
                };
                header
                    .child(
                        icon_button(
                            "search-toggle",
                            glyph,
                            copy::search_toggle_label(self.search_open),
                            true,
                            cx,
                        )
                        .aria_expanded(self.search_open)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.search_open = !this.search_open;
                            if this.search_open {
                                this.search
                                    .update(cx, |search, cx| search.focus(window, cx));
                            } else {
                                this.search
                                    .update(cx, |search, cx| search.set_value("", window, cx));
                                this.dispatch(AppAction::QueryFiltered(String::new()), cx);
                            }
                            cx.notify();
                        })),
                    )
                    .child(
                        div()
                            .id("folder-chip")
                            .role(Role::Button)
                            .aria_label(copy::CHANGE_FOLDER)
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .max_w(px(150.0))
                            .h(px(36.0))
                            .pl(px(10.0))
                            .pr(px(12.0))
                            .rounded_full()
                            .bg(palette.fill)
                            .text_color(palette.ink_2)
                            .ui_text(12.0, 400.0)
                            .cursor_pointer()
                            .hover(move |style| style.bg(fill_hover))
                            .keyboard_focus(palette.surface, cx)
                            .child(icon(Icon::Folder, 16.0, palette.ink_2))
                            .child(
                                div()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(folder_name.to_owned()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.store.update(cx, |store, cx| store.choose_folder(cx));
                            })),
                    )
            })
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------------------------
// Welcome, scanning, errors
// ---------------------------------------------------------------------------------------------

impl LibraryPanel {
    fn welcome(
        &self,
        progress: Option<ScanProgress>,
        error: Option<FolderError>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = AppTheme::get(cx);
        let palette = theme.palette;
        let tokens = theme.tokens;
        let reduce_motion = cx.reduce_motion();
        let title = div()
            .flex()
            .flex_col()
            .type_role(&tokens.type_scale.display)
            .text_color(palette.ink)
            .children(
                copy::WELCOME_TITLE
                    .split_inclusive('の')
                    .map(|line| div().child(line.to_owned())),
            );
        let lead = match error {
            None => div()
                .mt(px(14.0))
                .type_role(&tokens.type_scale.body)
                .text_color(palette.ink_2)
                .child(copy::WELCOME_DESCRIPTION)
                .into_any_element(),
            Some(error) => {
                let (headline, description) = copy::folder_error_text(error);
                div()
                    .id("folder-error")
                    .role(Role::Alert)
                    .mt(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .px(px(16.0))
                    .py(px(14.0))
                    .rounded(px(14.0))
                    .bg(palette.warning_soft)
                    .child(
                        div()
                            .ui_text(15.0, 600.0)
                            .line_height(relative(1.5))
                            .text_color(palette.ink)
                            .child(headline),
                    )
                    .child(
                        div()
                            .type_role(&tokens.type_scale.body)
                            .line_height(relative(1.7))
                            .text_color(palette.ink_2)
                            .child(description),
                    )
                    .into_any_element()
            }
        };
        let action = match progress {
            None => {
                let label = if error.is_some() {
                    copy::ERROR_RETRY
                } else {
                    copy::WELCOME_ACTION
                };
                div().mt(px(28.0)).flex().child(primary_button(
                    "choose-folder",
                    Icon::Folder,
                    label,
                    cx.listener(|this, _, _, cx| {
                        this.store.update(cx, |store, cx| store.choose_folder(cx));
                    }),
                    cx,
                ))
            }
            Some(progress) => {
                div()
                    .mt(px(26.0))
                    .child(self.scan_block(progress, &palette, reduce_motion, cx))
            }
        };
        let note = |glyph, (headline, body): (&'static str, &'static str)| {
            div()
                .flex()
                .gap(px(10.0))
                .child(div().w(px(32.0)).pt(px(2.0)).pl(px(6.0)).child(icon(
                    glyph,
                    20.0,
                    palette.ink,
                )))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .ui_text(13.0, 400.0)
                        .line_height(relative(1.7))
                        .text_color(palette.ink_2)
                        .child(
                            div()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(palette.ink)
                                .child(headline),
                        )
                        .child(body),
                )
        };
        div()
            .id("welcome")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .pt(px(28.0))
            .px(px(26.0))
            .pb(px(22.0))
            .overflow_y_scroll()
            .child(title)
            .child(lead)
            .child(action)
            .child(
                div()
                    .mt_auto()
                    .pt(px(28.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(note(Icon::Shield, copy::WELCOME_NOTES[0]))
                    .child(note(Icon::Download, copy::WELCOME_NOTES[1])),
            )
            .into_any_element()
    }

    fn scan_block(
        &self,
        progress: ScanProgress,
        palette: &Palette,
        reduce_motion: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scan = copy::scan_progress_copy(progress);
        let bar = div()
            .absolute()
            .top_0()
            .bottom_0()
            .w(relative(0.36))
            .rounded(px(2.0))
            .bg(palette.drive);
        let bar = if reduce_motion {
            bar.left_0().into_any_element()
        } else {
            bar.with_animation(
                "scan-bar",
                Animation::new(Duration::from_millis(1_400)).repeat(),
                |bar, delta| bar.left(relative(-0.36 + delta * 1.36)),
            )
            .into_any_element()
        };
        div()
            .id("scan-progress")
            .role(Role::Status)
            .flex()
            .flex_col()
            .gap(px(8.0))
            .pt(px(16.0))
            .px(px(18.0))
            .pb(px(18.0))
            .rounded(px(18.0))
            .bg(palette.fill)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mr(px(-10.0))
                    .child(
                        div()
                            .ui_text(13.0, 600.0)
                            .text_color(palette.ink)
                            .child(copy::SCAN_TITLE),
                    )
                    .child(text_button("cancel-scan", copy::SCAN_CANCEL, cx).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.store.update(cx, |store, cx| store.cancel_scan(cx));
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .ui_text(13.0, 500.0)
                    .text_color(palette.ink_2)
                    .when_some(scan.count, |line, count| {
                        line.child(
                            div()
                                .mr(px(2.0))
                                .numeral()
                                .text_size(px(24.0))
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(palette.ink)
                                .child(count),
                        )
                    })
                    .child(scan.label),
            )
            .child(
                div()
                    .relative()
                    .h(px(4.0))
                    .rounded(px(2.0))
                    .bg(palette.line)
                    .overflow_hidden()
                    .child(bar),
            )
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------------------------
// Library browser
// ---------------------------------------------------------------------------------------------

const CATEGORY_OPTIONS: [(CategoryFilter, Option<ClipCategory>); 4] = [
    (CategoryFilter::All, None),
    (
        CategoryFilter::Only(ClipCategory::Recent),
        Some(ClipCategory::Recent),
    ),
    (
        CategoryFilter::Only(ClipCategory::Saved),
        Some(ClipCategory::Saved),
    ),
    (
        CategoryFilter::Only(ClipCategory::Sentry),
        Some(ClipCategory::Sentry),
    ),
];

impl LibraryPanel {
    fn browser(&mut self, workspace: &Workspace, cx: &mut Context<Self>) -> AnyElement {
        let theme = AppTheme::get(cx);
        let palette = theme.palette;
        let tokens = theme.tokens;
        let day_key = workspace.timeline_date();
        if self
            .cluster
            .as_ref()
            .is_some_and(|cluster| Some(&cluster.day_key) != day_key.as_ref())
        {
            self.cluster = None;
        }
        let selected = workspace.selected_event_id.clone();
        let day_events = workspace.day_events();
        let rows: Vec<Row> = day_events
            .iter()
            .filter(|event| {
                self.cluster
                    .as_ref()
                    .is_none_or(|cluster| cluster.event_ids.contains(&event.id))
            })
            .map(|event| Row {
                id: event.id.clone(),
                copy: event_row(event),
                thumbnail: event
                    .thumbnail
                    .map(|file| workspace.library.file(file).path.clone()),
                selected: Some(&event.id) == selected.as_ref(),
            })
            .collect();
        let empty_text = workspace.empty_rows_text();
        self.rows = Rc::new(rows);
        let counts = workspace.category_counts();
        let timeline_events = workspace.timeline_events();
        let rail = build_day_rail(
            &timeline_events,
            day_key.as_deref(),
            f64::from(self.rail_width.get()),
        );
        while self.rail_focus.len() < rail.items.len() {
            self.rail_focus.push(cx.focus_handle().tab_stop(true));
        }
        let previous_day = workspace.adjacent_day(Step::Previous);
        let next_day = workspace.adjacent_day(Step::Next);
        let title = day_key
            .as_deref()
            .and_then(|day| parse_filename_local_timestamp(&format!("{day}_00-00-00")))
            .map(|date| copy::day_title_parts(&date));
        let show_search = self.search_open || !workspace.filters.query.is_empty();
        let category = workspace.filters.category;
        let warnings = workspace.library.warnings.clone();
        let days = workspace.timeline_days();

        let search = show_search.then(|| {
            div()
                .id("search-field")
                .mx(px(14.0))
                .mb(px(8.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .h(px(36.0))
                .px(px(12.0))
                .rounded_full()
                .bg(palette.fill)
                .child(icon(Icon::Search, 16.0, palette.ink_3))
                .child(
                    div()
                        .flex_1()
                        .ui_text(13.0, 500.0)
                        .text_color(palette.ink)
                        .child(
                            Input::new(&self.search)
                                .appearance(false)
                                .aria_label(copy::SEARCH_LABEL),
                        ),
                )
                .on_key_down(
                    cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape"
                            && this.search.read(cx).value().is_empty()
                        {
                            this.search_open = false;
                            window.blur(cx);
                            cx.notify();
                        }
                    }),
                )
        });

        let segmented = div()
            .id("categories")
            .role(Role::RadioGroup)
            .aria_label(copy::CATEGORY_GROUP)
            .mx(px(14.0))
            .mt(px(4.0))
            .flex()
            .gap(px(2.0))
            .p(px(3.0))
            .rounded(px(tokens.radii.segmented))
            .bg(palette.fill)
            .children(CATEGORY_OPTIONS.map(|(filter, category_value)| {
                let checked = filter == category;
                let count = counts.get(filter);
                let short = category_value.map_or(copy::CATEGORY_ALL, copy::category_short_label);
                let item = div()
                    .id((
                        "category",
                        category_value.map_or(0, |value| value as usize + 1),
                    ))
                    .role(Role::RadioButton)
                    .aria_label(copy::category_option_label(category_value, count))
                    .aria_toggled(if checked {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(1.0))
                    .pt(px(7.0))
                    .pb(px(6.0))
                    .px(px(4.0))
                    .rounded(px(11.0))
                    .ui_text(12.0, 400.0)
                    .text_color(palette.ink_2)
                    .cursor_pointer()
                    .keyboard_focus(palette.fill, cx)
                    .child(short)
                    .child(
                        div()
                            .numeral()
                            .text_size(px(15.0))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .line_height(relative(1.1))
                            .text_color(palette.ink)
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dispatch(AppAction::CategoryFiltered(filter), cx);
                    }));
                if checked {
                    segment_selected(item, cx)
                } else {
                    let ink = palette.ink;
                    item.hover(move |style| style.text_color(ink))
                }
            }));

        let day_button = |id: &'static str,
                          step: Step,
                          glyph,
                          label: String,
                          enabled: bool,
                          cx: &mut Context<Self>| {
            icon_button(id, glyph, label, enabled, cx).when(enabled, |button| {
                button.on_click(cx.listener(move |this, _, _, cx| {
                    this.picker_month = None;
                    this.dispatch(AppAction::DayStepped(step), cx);
                }))
            })
        };
        let previous_label = previous_day
            .as_ref()
            .map_or(copy::PREVIOUS_DAY.to_owned(), |day| {
                copy::day_button_label(true, &day.date_key)
            });
        let next_label = next_day.as_ref().map_or(copy::NEXT_DAY.to_owned(), |day| {
            copy::day_button_label(false, &day.date_key)
        });
        let picker = self.picker_month.map(|month| {
            let grid = month_grid(month, &days, day_key.as_deref());
            let entity = cx.entity().downgrade();
            deferred(
                anchored()
                    .position_mode(gpui_kit::AnchoredPositionMode::Local)
                    .position(point(px(0.0), px(44.0)))
                    .snap_to_window_with_margin(px(8.0))
                    .child(date_picker(
                        &grid,
                        move |action, _, cx| {
                            entity
                                .update(cx, |this, cx| {
                                    match action {
                                        PickerAction::Month(delta) => {
                                            this.picker_month = this
                                                .picker_month
                                                .and_then(|month| month.shifted(delta));
                                        }
                                        PickerAction::Day(date_key) => {
                                            this.picker_month = None;
                                            this.dispatch(AppAction::DayChosen(date_key), cx);
                                        }
                                        PickerAction::Close => this.picker_month = None,
                                    }
                                    cx.notify();
                                })
                                .ok();
                        },
                        cx,
                    )),
            )
            .with_priority(2)
        });
        let title_button = div()
            .id("day-title")
            .role(Role::Button)
            .aria_label(copy::date_picker_label(title.as_ref()))
            .aria_expanded(self.picker_month.is_some())
            .flex()
            .items_baseline()
            .max_w_full()
            .min_h(px(36.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(10.0))
            .whitespace_nowrap()
            .cursor_pointer()
            .keyboard_focus(palette.surface, cx)
            .hover({
                let fill = palette.fill;
                move |style| style.bg(fill)
            })
            .text_color(palette.ink)
            .type_role(&tokens.type_scale.title)
            .map(|button| match &title {
                Some(title) => button
                    .child(numeral_text(&title.date, 22.0, 600.0, None))
                    .child(
                        div()
                            .ml(px(6.0))
                            .ui_text(13.0, 500.0)
                            .text_color(palette.ink_2)
                            .child(title.weekday),
                    ),
                None => button.child(copy::DAY_NONE),
            })
            .on_click(cx.listener(|this, _, _, cx| {
                if this.picker_month.is_some() {
                    this.picker_month = None;
                    cx.notify();
                } else {
                    this.open_date_picker(cx);
                }
            }));
        let day = div()
            .pt(px(16.0))
            .px(px(20.0))
            .pb(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .mx(px(-8.0))
                    .child(
                        day_button(
                            "previous-day",
                            Step::Previous,
                            Icon::ChevronLeft,
                            previous_label,
                            previous_day.is_some(),
                            cx,
                        )
                        .test_support(),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .child(title_button)
                            .children(picker),
                    )
                    .child(
                        day_button(
                            "next-day",
                            Step::Next,
                            Icon::ChevronRight,
                            next_label,
                            next_day.is_some(),
                            cx,
                        )
                        .test_support(),
                    ),
            )
            .when(!rail.markers.is_empty(), |day| {
                let entity = cx.entity().downgrade();
                let day_for_cluster = day_key.clone().unwrap_or_default();
                let expanded = self.cluster.as_ref().map(|cluster| cluster.id.clone());
                day.child(day_rail(
                    &rail,
                    selected.as_deref(),
                    expanded.as_deref(),
                    self.rail_width.clone(),
                    &self.rail_focus,
                    move |click, _, cx| {
                        entity
                            .update(cx, |this, cx| match click {
                                RailClick::Event(event_id) => {
                                    this.dispatch(AppAction::EventSelected(event_id), cx);
                                }
                                RailClick::Cluster(cluster) => {
                                    this.cluster = if this
                                        .cluster
                                        .as_ref()
                                        .is_some_and(|open| open.id == cluster.id())
                                    {
                                        None
                                    } else {
                                        Some(open_cluster(&day_for_cluster, &cluster))
                                    };
                                    cx.notify();
                                }
                            })
                            .ok();
                    },
                    cx,
                ))
            });

        let cluster_bar = self.cluster.as_ref().map(|cluster| {
            div()
                .id("cluster-filter")
                .role(Role::Group)
                .aria_label(cluster.label.clone())
                .mx(px(14.0))
                .mt(px(8.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .pl(px(14.0))
                .pr(px(4.0))
                .rounded_full()
                .bg(palette.drive_soft)
                .ui_text(13.0, 600.0)
                .text_color(palette.ink)
                .child(numeral_text(&cluster.label, 13.0, 600.0, None))
                .child(
                    icon_button("clear-cluster", Icon::X, copy::CLUSTER_CLEAR, true, cx).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.cluster = None;
                            cx.notify();
                        }),
                    ),
                )
        });

        let list = if self.rows.is_empty() {
            // Fills the list's place so the warnings footer stays at the panel bottom.
            div()
                .flex_1()
                .min_h_0()
                .pt(px(18.0))
                .px(px(20.0))
                .type_role(&tokens.type_scale.ui)
                .text_color(palette.ink_2)
                .child(empty_text)
                .into_any_element()
        } else {
            image_cache(self.thumbnails.clone())
                .flex_1()
                .min_h_0()
                .mt(px(6.0))
                .child(
                    uniform_list(
                        "events",
                        self.rows.len(),
                        cx.processor(|this, range: Range<usize>, _, cx| {
                            let rows = this.rows.clone();
                            range
                                .map(|index| this.row(index, &rows[index], cx))
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(&self.list)
                    .size_full()
                    .pt(px(4.0))
                    .px(px(10.0))
                    .pb(px(12.0)),
                )
                .into_any_element()
        };

        let footer =
            (!warnings.is_empty()).then(|| {
                div()
                    .id("scan-warnings")
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .pt(px(10.0))
                    .px(px(20.0))
                    .pb(px(14.0))
                    .border_t_1()
                    .border_color(palette.line)
                    .ui_text(11.0, 400.0)
                    .line_height(relative(1.4))
                    .text_color(palette.ink_3)
                    .children(warnings.iter().map(|warning| {
                        div().child(copy::warning_text(warning.kind, warning.count))
                    }))
                    .test_support()
            });

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .children(search)
            .child(segmented)
            .child(day)
            .children(cluster_bar)
            .child(list)
            .children(footer)
            .into_any_element()
    }

    fn row(&self, index: usize, row: &Row, cx: &mut Context<Self>) -> AnyElement {
        let theme = AppTheme::get(cx);
        let palette = theme.palette;
        let tokens = theme.tokens;
        let fill = palette.fill;
        let event_id = row.id.clone();
        let thumbnail = div()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .w(px(tokens.sizes.thumbnail_width))
            .h(px(tokens.sizes.thumbnail_width * 3.0 / 4.0))
            .rounded(px(tokens.radii.thumbnail))
            .overflow_hidden()
            .bg(palette.video_missing_a)
            .child(icon(
                category_icon(row.copy.category),
                20.0,
                palette.video_muted,
            ))
            .when_some(row.thumbnail.clone(), |thumbnail, path| {
                thumbnail.child(
                    img(path)
                        .absolute()
                        .size_full()
                        .object_fit(gpui_kit::ObjectFit::Cover),
                )
            });
        let facts = div()
            .flex()
            .flex_wrap()
            .gap_x(px(12.0))
            .gap_y(px(2.0))
            .ui_text(12.0, 400.0)
            .line_height(relative(1.35))
            .text_color(palette.ink_2)
            .child(numeral_text(
                &row.copy.duration,
                13.0,
                500.0,
                Some(palette.ink),
            ))
            .child(numeral_text(
                &row.copy.cameras,
                13.0,
                500.0,
                Some(palette.ink),
            ))
            .when_some(row.copy.detection.clone(), |facts, detection| {
                facts.child(detection)
            })
            .when_some(row.copy.missing.clone(), |facts, missing| {
                facts.child(
                    div()
                        .w_full()
                        .text_size(px(11.0))
                        .text_color(palette.warning)
                        .child(missing),
                )
            });
        div()
            .h(px(ROW_STRIDE))
            .pb(px(2.0))
            .child(
                div()
                    .id(("event-row", index))
                    .role(Role::Button)
                    .aria_label(row.copy.label.clone())
                    .aria_selected(row.selected)
                    .size_full()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .p(px(8.0))
                    .rounded(px(tokens.radii.row))
                    .cursor_pointer()
                    .keyboard_focus(palette.surface, cx)
                    .map(|row_element| {
                        if row.selected {
                            row_element.bg(palette.drive_soft)
                        } else {
                            row_element.hover(move |style| style.bg(fill))
                        }
                    })
                    .child(thumbnail)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .type_role(&tokens.type_scale.numeral_l)
                                            .text_color(palette.ink)
                                            .child(row.copy.start_clock.clone()),
                                    )
                                    .child(category_chip(row.copy.category, ChipSize::Regular, cx)),
                            )
                            .child(facts),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.dispatch(AppAction::EventSelected(event_id.clone()), cx);
                    })),
            )
            .into_any_element()
    }
}

impl Render for LibraryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.clone();
        let state = store.read(cx).state().clone();
        let folder_name = match &state {
            AppState::Ready(workspace) => Some(workspace.folder_name.clone()),
            _ => None,
        };
        let header = self.header(folder_name.as_deref(), cx);
        let body = match &state {
            AppState::Idle => self.welcome(None, None, cx),
            AppState::Scanning { progress, .. } => self.welcome(Some(*progress), None, cx),
            AppState::Error { reason } => self.welcome(None, Some(*reason), cx),
            AppState::Ready(workspace) => self.browser(workspace, cx),
        };
        glass(AppTheme::get(cx).tokens.radii.panel, cx)
            .id("library")
            .role(Role::Group)
            .aria_label(copy::LIBRARY_REGION)
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(header)
            .child(body)
            .test_support()
    }
}
