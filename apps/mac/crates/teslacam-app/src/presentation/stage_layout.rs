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
}
