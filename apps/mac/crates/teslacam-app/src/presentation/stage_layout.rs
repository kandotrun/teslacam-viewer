//! Where the camera windows, the car and the field-of-view wedges go on the stage (`DESIGN.md` §5,
//! §6 Stage), computed from the stage's size and the shared stage geometry of `design-tokens.json`.
//!
//! Pure geometry in points with a top-left origin, like the web's CSS grid: a 3 × 3 grid with
//! fractional rows and columns, windows `contain`-fitted at 4:3 and centred in their cells, the
//! back window at most 86 % of its column, the car (1:2) filling the middle row.

use teslacam_core::camera::{CameraId, CameraMap};
use teslacam_core::theme::{StageArea, StageGeometry};

/// Padding inside the stage: 4 pt top and bottom, 24 pt left and right.
pub const STAGE_PADDING: (f32, f32) = (4.0, 24.0);
/// Window aspect ratio (width / height).
pub const WINDOW_ASPECT: f32 = 4.0 / 3.0;

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// A rectangle from its origin and size.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge.
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Centre point.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// The rectangle shrunk by `vertical` and `horizontal` on each side.
    pub fn inset(&self, vertical: f32, horizontal: f32) -> Rect {
        Rect::new(
            self.x + horizontal,
            self.y + vertical,
            (self.width - 2.0 * horizontal).max(0.0),
            (self.height - 2.0 * vertical).max(0.0),
        )
    }
}

/// A soft triangle from a camera's mount point to the two window corners that face the car; the
/// gradient runs from `mount` to `edge_midpoint`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wedge {
    /// The mount point on the car.
    pub mount: (f32, f32),
    /// The two corners of the facing edge.
    pub corners: [(f32, f32); 2],
    /// The middle of the facing edge (where the gradient ends).
    pub edge_midpoint: (f32, f32),
}

/// Side edges are inset 10 pt vertically, top and bottom edges 14 pt horizontally (prototype).
const VERTICAL_EDGE_INSET: f32 = 10.0;
const HORIZONTAL_EDGE_INSET: f32 = 14.0;

/// The wedge from `mount` to `window` (the web's `fieldOfViewWedge`).
pub fn field_of_view_wedge(mount: (f32, f32), window: Rect) -> Wedge {
    let (center_x, center_y) = window.center();
    let corners = if (center_x - mount.0).abs() > (center_y - mount.1).abs() {
        let edge_x = if center_x < mount.0 {
            window.right()
        } else {
            window.x
        };
        [
            (edge_x, window.y + VERTICAL_EDGE_INSET),
            (edge_x, window.bottom() - VERTICAL_EDGE_INSET),
        ]
    } else {
        let edge_y = if center_y < mount.1 {
            window.bottom()
        } else {
            window.y
        };
        [
            (window.x + HORIZONTAL_EDGE_INSET, edge_y),
            (window.right() - HORIZONTAL_EDGE_INSET, edge_y),
        ]
    };
    Wedge {
        mount,
        corners,
        edge_midpoint: (
            (corners[0].0 + corners[1].0) / 2.0,
            (corners[0].1 + corners[1].1) / 2.0,
        ),
    }
}

/// The cameras around the car.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleLayout {
    /// The fitted window of each camera.
    pub windows: CameraMap<Rect>,
    /// The car image box (1:2).
    pub car: Rect,
    /// Each camera's mount point on the car.
    pub mounts: CameraMap<(f32, f32)>,
    /// Each camera's wedge.
    pub wedges: CameraMap<Wedge>,
}

fn tracks(start: f32, length: f32, fractions: [f32; 3], gap: f32) -> [(f32, f32); 3] {
    let total: f32 = fractions.iter().sum();
    let free = (length - 2.0 * gap).max(0.0);
    let mut cursor = start;
    fractions.map(|fraction| {
        let size = free * fraction / total;
        let track = (cursor, size);
        cursor += size + gap;
        track
    })
}

/// The cell an area spans in the 3 × 3 grid (it may cover several rows or columns).
fn area_cell(
    geometry: &StageGeometry,
    area: StageArea,
    rows: &[(f32, f32); 3],
    columns: &[(f32, f32); 3],
) -> Rect {
    let mut first = (usize::MAX, usize::MAX);
    let mut last = (0, 0);
    for (row, cells) in geometry.areas.iter().enumerate() {
        for (column, cell) in cells.iter().enumerate() {
            if *cell == area {
                first = (first.0.min(row), first.1.min(column));
                last = (last.0.max(row), last.1.max(column));
            }
        }
    }
    if first.0 == usize::MAX {
        return Rect::default();
    }
    let top = rows[first.0].0;
    let left = columns[first.1].0;
    Rect::new(
        left,
        top,
        columns[last.1].0 + columns[last.1].1 - left,
        rows[last.0].0 + rows[last.0].1 - top,
    )
}

/// A 4:3 window `contain`-fitted into `cell` (at most `max_width_ratio` of its width), centred.
pub fn fit_window(cell: Rect, max_width_ratio: f32) -> Rect {
    let width = (cell.width * max_width_ratio).min(cell.height * WINDOW_ASPECT);
    let height = width / WINDOW_ASPECT;
    Rect::new(
        cell.x + (cell.width - width) / 2.0,
        cell.y + (cell.height - height) / 2.0,
        width,
        height,
    )
}

/// The vehicle layout inside `stage` (the stage's full box; padding is applied here).
pub fn vehicle_layout(stage: Rect, geometry: &StageGeometry) -> VehicleLayout {
    let inner = stage.inset(STAGE_PADDING.0, STAGE_PADDING.1);
    let rows = tracks(inner.y, inner.height, geometry.rows, geometry.row_gap);
    let columns = tracks(inner.x, inner.width, geometry.columns, geometry.column_gap);
    let windows = CameraMap::from_fn(|camera| {
        let cell = area_cell(geometry, StageArea::Camera(camera), &rows, &columns);
        let ratio = if camera == CameraId::Back {
            geometry.back_width_ratio
        } else {
            1.0
        };
        fit_window(cell, ratio)
    });
    let car_cell = area_cell(geometry, StageArea::Vehicle, &rows, &columns);
    let car_height = car_cell.height.min(car_cell.width * 2.0);
    let car = Rect::new(
        car_cell.x + (car_cell.width - car_height / 2.0) / 2.0,
        car_cell.y + (car_cell.height - car_height) / 2.0,
        car_height / 2.0,
        car_height,
    );
    let mounts = geometry
        .mounts
        .map(|_, (x, y)| (car.x + car.width * x, car.y + car.height * y));
    let wedges = CameraMap::from_fn(|camera| field_of_view_wedge(mounts[camera], windows[camera]));
    VehicleLayout {
        windows,
        car,
        mounts,
        wedges,
    }
}

/// The 6分割 grid: 3 × 2 cells in reading order front, back, left pillar / right pillar, left
/// repeater, right repeater, each window `contain`-fitted.
pub fn grid_layout(stage: Rect, geometry: &StageGeometry) -> CameraMap<Rect> {
    const ORDER: [CameraId; 6] = [
        CameraId::Front,
        CameraId::Back,
        CameraId::LeftPillar,
        CameraId::RightPillar,
        CameraId::LeftRepeater,
        CameraId::RightRepeater,
    ];
    let inner = stage.inset(STAGE_PADDING.0, STAGE_PADDING.1);
    let cell_width = (inner.width - 2.0 * geometry.column_gap).max(0.0) / 3.0;
    let cell_height = (inner.height - geometry.row_gap).max(0.0) / 2.0;
    CameraMap::from_fn(|camera| {
        let index = ORDER
            .iter()
            .position(|candidate| *candidate == camera)
            .unwrap_or(0);
        let (row, column) = (index / 3, index % 3);
        fit_window(
            Rect::new(
                inner.x + column as f32 * (cell_width + geometry.column_gap),
                inner.y + row as f32 * (cell_height + geometry.row_gap),
                cell_width,
                cell_height,
            ),
            1.0,
        )
    })
}

/// Gap between the main window and the strip, and between strip cells.
pub const STRIP_GAP: f32 = 14.0;
/// The strip's height range and its share of the window height (`clamp(72px, 15vh, 128px)`).
pub const STRIP_HEIGHT: (f32, f32, f32) = (72.0, 0.15, 128.0);
/// Width of the mini car: columns 36 | 30 | 36, 2 pt gaps, 4 pt padding.
pub const SWITCHER_WIDTH: f32 = 4.0 + 36.0 + 2.0 + 30.0 + 2.0 + 36.0 + 4.0;
/// Windows at most this tall use the compact chrome (smaller name pill, icon-only states).
pub const COMPACT_WINDOW_HEIGHT: f32 = 120.0;

/// Whether `window` is small enough for the compact chrome.
pub fn compact_chrome(window: Rect) -> bool {
    window.height <= COMPACT_WINDOW_HEIGHT
}

fn strip_height(viewport_height: f32) -> f32 {
    let (min, share, max) = STRIP_HEIGHT;
    (viewport_height * share).clamp(min, max)
}

/// Splits `row` into `count` equal cells `gap` apart.
fn cells(row: Rect, count: usize, gap: f32) -> Vec<Rect> {
    let width = ((row.width - gap * (count as f32 - 1.0)) / count as f32).max(0.0);
    (0..count)
        .map(|index| {
            Rect::new(
                row.x + index as f32 * (width + gap),
                row.y,
                width,
                row.height,
            )
        })
        .collect()
}

/// Where a `source`-sized video is drawn in a window with `radius` corners: `contain`-fitted and
/// centred, and, when its corners would stick out of the rounded window (GPUI draws video surfaces
/// without rounded corners), shrunk until they touch the rounding. Wide footage is letterboxed
/// edge to edge; 4:3 footage keeps a hairline of the window's black around it. An unknown size
/// counts as 4:3.
pub fn video_rect(window: Rect, source: (f64, f64), radius: f32) -> Rect {
    let (width, height) = if source.0 > 0.0 && source.1 > 0.0 {
        (source.0 as f32, source.1 as f32)
    } else {
        (4.0, 3.0)
    };
    let fit = (window.width / width).min(window.height / height);
    let (width, height) = (width * fit, height * fit);
    let at = |scale: f32| {
        let (w, h) = (width * scale, height * scale);
        Rect::new(
            window.x + (window.width - w) / 2.0,
            window.y + (window.height - h) / 2.0,
            w,
            h,
        )
    };
    let corner_inside = |rect: Rect| {
        let (dx, dy) = (rect.x - window.x, rect.y - window.y);
        dx >= radius || dy >= radius || (radius - dx).hypot(radius - dy) <= radius
    };
    if corner_inside(at(1.0)) {
        return at(1.0);
    }
    let (mut inside, mut outside) = (0.0f32, 1.0f32);
    for _ in 0..32 {
        let middle = (inside + outside) / 2.0;
        if corner_inside(at(middle)) {
            inside = middle;
        } else {
            outside = middle;
        }
    }
    at(inside)
}

/// One camera filling the stage, the other five in a strip under it, the mini car at the strip's
/// left.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedLayout {
    /// The expanded camera's window (4:3, centred above the strip).
    pub main: Rect,
    /// The other five windows in canonical order.
    pub strip: Vec<(CameraId, Rect)>,
    /// The mini car's box.
    pub switcher: Rect,
    /// Height of the strip row.
    pub strip_height: f32,
}

/// The expanded layout of `focus` inside `stage` in a window `viewport_height` points tall.
pub fn expanded_layout(stage: Rect, focus: CameraId, viewport_height: f32) -> ExpandedLayout {
    let inner = stage.inset(STAGE_PADDING.0, STAGE_PADDING.1);
    let strip_height = strip_height(viewport_height).min(inner.height);
    let main_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        (inner.height - strip_height - STRIP_GAP).max(0.0),
    );
    let row = Rect::new(
        inner.x,
        inner.bottom() - strip_height,
        inner.width,
        strip_height,
    );
    let switcher = Rect::new(row.x, row.y, SWITCHER_WIDTH.min(row.width), row.height);
    let rest = Rect::new(
        row.x + SWITCHER_WIDTH + STRIP_GAP,
        row.y,
        (row.width - SWITCHER_WIDTH - STRIP_GAP).max(0.0),
        row.height,
    );
    let others: Vec<CameraId> = CameraId::ALL
        .into_iter()
        .filter(|camera| *camera != focus)
        .collect();
    let strip = others
        .iter()
        .zip(cells(rest, others.len(), STRIP_GAP))
        .map(|(camera, cell)| (*camera, fit_window(cell, 1.0)))
        .collect();
    ExpandedLayout {
        main: fit_window(main_area, 1.0),
        strip,
        switcher,
        strip_height,
    }
}

/// The mini car: the car glyph with one 36 pt button per camera around it.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitcherLayout {
    /// The car image box (1:2).
    pub car: Rect,
    /// Each camera's dot centre.
    pub dots: CameraMap<(f32, f32)>,
    /// Each camera's hit target.
    pub buttons: CameraMap<Rect>,
}

/// The mini car inside `area` (a [`SWITCHER_WIDTH`] wide strip cell): front and back above and
/// below the car, the pillars at the top corners, the repeaters at the bottom corners.
pub fn switcher_layout(area: Rect) -> SwitcherLayout {
    const BUTTON: f32 = 36.0;
    const CAR_COLUMN: f32 = 30.0;
    const CAR_MAX_HEIGHT: f32 = 140.0;
    let left = area.x + 4.0 + BUTTON / 2.0;
    let centre = area.x + 4.0 + BUTTON + 2.0 + CAR_COLUMN / 2.0;
    let right = area.x + 4.0 + BUTTON + 2.0 + CAR_COLUMN + 2.0 + BUTTON / 2.0;
    let top = area.y + BUTTON / 2.0;
    let bottom = area.bottom() - BUTTON / 2.0;
    let dots = CameraMap::from_fn(|camera| match camera {
        CameraId::Front => (centre, top),
        CameraId::Back => (centre, bottom),
        CameraId::LeftPillar => (left, top),
        CameraId::RightPillar => (right, top),
        CameraId::LeftRepeater => (left, bottom),
        CameraId::RightRepeater => (right, bottom),
    });
    let middle = (area.height - 2.0 * BUTTON).clamp(0.0, CAR_MAX_HEIGHT);
    let car_height = middle.min(CAR_COLUMN * 2.0);
    let car = Rect::new(
        centre - car_height / 4.0,
        area.y + BUTTON + ((area.height - 2.0 * BUTTON).max(0.0) - car_height) / 2.0,
        car_height / 2.0,
        car_height,
    );
    SwitcherLayout {
        car,
        buttons: dots
            .map(|_, (x, y)| Rect::new(x - BUTTON / 2.0, y - BUTTON / 2.0, BUTTON, BUTTON)),
        dots,
    }
}

/// `event.mp4` filling the stage above a strip of all six cameras.
#[derive(Debug, Clone, PartialEq)]
pub struct RepresentativeLayout {
    /// The representative video's box (not 4:3: the video is `contain`-fitted inside).
    pub main: Rect,
    /// The six windows in canonical order.
    pub strip: Vec<(CameraId, Rect)>,
}

/// The representative layout inside `stage` in a window `viewport_height` points tall.
pub fn representative_layout(stage: Rect, viewport_height: f32) -> RepresentativeLayout {
    let inner = stage.inset(STAGE_PADDING.0, STAGE_PADDING.1);
    let strip_height = strip_height(viewport_height).min(inner.height);
    let row = Rect::new(
        inner.x,
        inner.bottom() - strip_height,
        inner.width,
        strip_height,
    );
    RepresentativeLayout {
        main: Rect::new(
            inner.x,
            inner.y,
            inner.width,
            (inner.height - strip_height - STRIP_GAP).max(0.0),
        ),
        strip: CameraId::ALL
            .into_iter()
            .zip(cells(row, 6, STRIP_GAP))
            .map(|(camera, cell)| (camera, fit_window(cell, 1.0)))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use teslacam_core::theme::Theme;

    use super::*;

    fn geometry() -> StageGeometry {
        Theme::embedded().stage.clone()
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.01
    }

    #[test]
    fn windows_are_four_by_three_and_inside_the_stage() {
        let stage = Rect::new(404.0, 76.0, 1020.0, 700.0);
        let layout = vehicle_layout(stage, &geometry());
        for (camera, window) in layout.windows.iter() {
            assert!(close(window.width / window.height, 4.0 / 3.0), "{camera:?}");
            assert!(
                window.x >= stage.x && window.right() <= stage.right() + 0.01,
                "{camera:?}"
            );
            assert!(
                window.y >= stage.y && window.bottom() <= stage.bottom() + 0.01,
                "{camera:?}"
            );
        }
        // The front window is centred above the car, the back one below it, the pillars beside it.
        let front = layout.windows[CameraId::Front];
        let back = layout.windows[CameraId::Back];
        assert!(close(front.center().0, layout.car.center().0));
        assert!(close(back.center().0, layout.car.center().0));
        assert!(front.bottom() <= layout.car.y + 0.01);
        assert!(back.y >= layout.car.bottom() - 0.01);
        assert!(layout.windows[CameraId::LeftPillar].right() < layout.car.x);
        assert!(layout.windows[CameraId::RightPillar].x > layout.car.right());
        assert!(close(layout.car.width * 2.0, layout.car.height));
    }

    #[test]
    fn rows_and_columns_follow_the_token_fractions() {
        // 1092 × 728 inner box: columns (1092 − 52) × [1, 1.18, 1] / 3.18, rows (728 − 28) × [.8, 1.12, .86] / 2.78.
        let stage = Rect::new(0.0, 0.0, 1140.0, 736.0);
        let layout = vehicle_layout(stage, &geometry());
        let column: f32 = 1040.0 / 3.18;
        let rows = [0.8f32, 1.12, 0.86].map(|fraction| 700.0 * fraction / 2.78);
        // The car fills the middle row.
        assert!(close(layout.car.height, rows[1]));
        assert!(close(layout.car.y, 4.0 + rows[0] + 14.0));
        // The left pillar spans the first two rows and is centred in them.
        let pillar = layout.windows[CameraId::LeftPillar];
        let span = (4.0, rows[0] + 14.0 + rows[1]);
        assert!(close(pillar.center().1, span.0 + span.1 / 2.0));
        assert!(close(pillar.width, column.min(span.1 * 4.0 / 3.0)));
        // The back window is at most 86 % of its column.
        let back = layout.windows[CameraId::Back];
        assert!(back.width <= 0.86 * column * 1.18 + 0.01);
    }

    #[test]
    fn mounts_sit_on_the_car_and_wedges_face_it() {
        let layout = vehicle_layout(Rect::new(0.0, 0.0, 1100.0, 720.0), &geometry());
        let car = layout.car;
        let (x, y) = layout.mounts[CameraId::Front];
        assert!(close(x, car.x + car.width * 0.5) && close(y, car.y + car.height * 0.22));
        // The front window is above the car: its wedge ends on the window's bottom edge.
        let front = layout.wedges[CameraId::Front];
        let window = layout.windows[CameraId::Front];
        assert!(close(front.corners[0].1, window.bottom()));
        assert!(close(front.corners[0].0, window.x + 14.0));
        assert!(
            close(front.edge_midpoint.0, front.mount.0) && front.edge_midpoint.1 < front.mount.1
        );
        // The left repeater is to the lower left: its wedge ends on the window's right edge.
        let repeater = layout.wedges[CameraId::LeftRepeater];
        let window = layout.windows[CameraId::LeftRepeater];
        assert!(close(repeater.corners[0].0, window.right()));
        assert!(close(repeater.corners[1].1, window.bottom() - 10.0));
        // It fans out to the lower left of its mount point.
        assert!(repeater.edge_midpoint.0 < repeater.mount.0);
        assert!(repeater.edge_midpoint.1 > repeater.mount.1);
    }

    #[test]
    fn the_wedge_is_the_prototype_triangle() {
        let window = Rect::new(0.0, 0.0, 100.0, 75.0);
        let wedge = field_of_view_wedge((300.0, 40.0), window);
        assert_eq!(wedge.corners, [(100.0, 10.0), (100.0, 65.0)]);
        assert_eq!(wedge.edge_midpoint, (100.0, 37.5));
        let below = field_of_view_wedge((50.0, -200.0), window);
        assert_eq!(below.corners, [(14.0, 0.0), (86.0, 0.0)]);
        assert_eq!(below.edge_midpoint, (50.0, 0.0));
    }

    #[test]
    fn the_grid_reads_front_back_pillars_then_repeaters() {
        let windows = grid_layout(Rect::new(0.0, 0.0, 1100.0, 700.0), &geometry());
        let front = windows[CameraId::Front];
        let back = windows[CameraId::Back];
        let left_pillar = windows[CameraId::LeftPillar];
        let right_pillar = windows[CameraId::RightPillar];
        let right_repeater = windows[CameraId::RightRepeater];
        assert!(close(front.y, back.y) && close(back.y, left_pillar.y));
        assert!(front.x < back.x && back.x < left_pillar.x);
        assert!(right_pillar.y > front.bottom());
        assert!(close(right_pillar.x, front.x));
        assert!(close(right_repeater.x, left_pillar.x));
        assert!(close(front.width / front.height, 4.0 / 3.0));
    }

    #[test]
    fn an_expanded_camera_fills_the_stage_above_a_strip_of_the_other_five() {
        let stage = Rect::new(0.0, 0.0, 1020.0, 700.0);
        let layout = expanded_layout(stage, CameraId::Back, 900.0);
        // The strip is clamp(72, 15 % of the window height, 128) = 128 tall at the bottom of the
        // inner box (4 pt padding), 14 pt below the main area.
        assert!(close(layout.strip_height, 128.0));
        assert!(close(layout.switcher.bottom(), 696.0));
        assert!(close(layout.switcher.height, 128.0));
        assert!(close(layout.switcher.width, SWITCHER_WIDTH));
        assert!(close(layout.switcher.x, 24.0));
        // The main window is 4:3, centred in the area above the strip.
        let main = layout.main;
        assert!(close(main.width / main.height, 4.0 / 3.0));
        assert!(close(main.y, 4.0));
        assert!(close(main.height, 696.0 - 128.0 - 14.0 - 4.0));
        assert!(close(main.center().0, 510.0));
        // The other five, in canonical order, left to right after the switcher.
        let order: Vec<CameraId> = layout.strip.iter().map(|(camera, _)| *camera).collect();
        assert_eq!(
            order,
            [
                CameraId::Front,
                CameraId::LeftPillar,
                CameraId::LeftRepeater,
                CameraId::RightPillar,
                CameraId::RightRepeater
            ]
        );
        let cell = (1020.0 - 48.0 - SWITCHER_WIDTH - 5.0 * 14.0) / 5.0;
        for (index, (_, window)) in layout.strip.iter().enumerate() {
            assert!(close(window.width / window.height, 4.0 / 3.0));
            assert!(window.width <= cell + 0.01 && window.height <= 128.01);
            let cell_x = 24.0 + SWITCHER_WIDTH + 14.0 + index as f32 * (cell + 14.0);
            assert!(close(window.center().0, cell_x + cell / 2.0));
            assert!(close(window.center().1, 696.0 - 64.0));
        }
        // A short window: 15 % of 700 pt = 105 pt.
        let short = expanded_layout(stage, CameraId::Front, 700.0);
        assert!(close(short.strip_height, 105.0));
        assert!(close(
            expanded_layout(stage, CameraId::Front, 400.0).strip_height,
            72.0
        ));
    }

    #[test]
    fn the_mini_car_puts_each_dot_where_its_camera_sits() {
        let switcher = switcher_layout(Rect::new(24.0, 568.0, SWITCHER_WIDTH, 128.0));
        let dot = |camera| switcher.dots[camera];
        // Columns 36 | 30 | 36 with 2 pt gaps and 4 pt padding; rows 36 | rest | 36.
        assert_eq!(dot(CameraId::LeftPillar), (24.0 + 22.0, 568.0 + 18.0));
        assert_eq!(dot(CameraId::Front), (24.0 + 57.0, 568.0 + 18.0));
        assert_eq!(dot(CameraId::RightPillar), (24.0 + 92.0, 568.0 + 18.0));
        assert_eq!(dot(CameraId::LeftRepeater), (24.0 + 22.0, 568.0 + 110.0));
        assert_eq!(dot(CameraId::Back), (24.0 + 57.0, 568.0 + 110.0));
        assert_eq!(dot(CameraId::RightRepeater), (24.0 + 92.0, 568.0 + 110.0));
        // The car (1:2) fills the middle row inside the 30 pt column.
        let car = switcher.car;
        assert!(close(car.height, 56.0) && close(car.width, 28.0));
        assert!(close(car.center().0, 24.0 + 57.0));
        assert!(close(car.center().1, 568.0 + 64.0));
        // A taller strip is limited by the column width.
        let tall = switcher_layout(Rect::new(0.0, 0.0, SWITCHER_WIDTH, 200.0));
        assert!(close(tall.car.width, 30.0) && close(tall.car.height, 60.0));
        // Hit targets are 36 pt squares centred on the dots.
        let button = switcher.buttons[CameraId::Back];
        assert!(close(button.width, 36.0) && close(button.center().0, 24.0 + 57.0));
    }

    #[test]
    fn the_representative_video_fills_the_stage_above_all_six_cameras() {
        let stage = Rect::new(0.0, 0.0, 1020.0, 700.0);
        let layout = representative_layout(stage, 900.0);
        assert_eq!(
            layout.main,
            Rect::new(24.0, 4.0, 972.0, 696.0 - 128.0 - 14.0 - 4.0)
        );
        let order: Vec<CameraId> = layout.strip.iter().map(|(camera, _)| *camera).collect();
        assert_eq!(order, CameraId::ALL);
        let cell = (972.0 - 5.0 * 14.0) / 6.0;
        for (index, (_, window)) in layout.strip.iter().enumerate() {
            assert!(close(window.width / window.height, 4.0 / 3.0));
            assert!(close(
                window.center().0,
                24.0 + index as f32 * (cell + 14.0) + cell / 2.0
            ));
        }
    }

    #[test]
    fn small_windows_use_the_compact_chrome() {
        assert!(compact_chrome(Rect::new(0.0, 0.0, 160.0, 120.0)));
        assert!(!compact_chrome(Rect::new(0.0, 0.0, 170.0, 128.0)));
    }

    fn inside_rounded(window: Rect, radius: f32, (x, y): (f32, f32)) -> bool {
        let cx = x.clamp(window.x + radius, window.right() - radius);
        let cy = y.clamp(window.y + radius, window.bottom() - radius);
        (x - cx).hypot(y - cy) <= radius + 0.01
    }

    #[test]
    fn wide_video_is_letterboxed_edge_to_edge() {
        // Tesla HW4 side cameras (1448 × 938) in a 400 × 300 window: full width, bars top and bottom.
        let window = Rect::new(10.0, 20.0, 400.0, 300.0);
        let video = video_rect(window, (1448.0, 938.0), 14.0);
        assert!(close(video.width, 400.0));
        assert!(close(video.height, 400.0 * 938.0 / 1448.0));
        assert!(close(video.center().1, window.center().1));
    }

    #[test]
    fn four_by_three_video_shrinks_until_its_corners_touch_the_rounding() {
        let window = Rect::new(10.0, 20.0, 400.0, 300.0);
        let video = video_rect(window, (1280.0, 960.0), 14.0);
        assert!(close(video.width / video.height, 4.0 / 3.0));
        assert!(close(video.center().0, window.center().0));
        assert!(close(video.center().1, window.center().1));
        assert!(video.width < 400.0 && video.width > 390.0, "{video:?}");
        // The corners lie on the rounded edge, not outside it.
        assert!(inside_rounded(window, 14.0, (video.x, video.y)));
        assert!(!inside_rounded(
            window,
            14.0,
            (video.x - 0.5, video.y - 0.5)
        ));
        // Without a known size the window is filled the same way.
        assert_eq!(video_rect(window, (0.0, 0.0), 14.0), video);
        // A tall video in a 4:3 window: pillarboxed, corners well inside.
        let tall = video_rect(window, (720.0, 1280.0), 14.0);
        assert!(close(tall.height, 300.0));
    }
}
