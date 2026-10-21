//! Camera ids, clip categories and per-camera maps.

use std::ops::{Index, IndexMut};

/// One of the six TeslaCam cameras, in canonical order (`front`, `back`, `left_pillar`,
/// `left_repeater`, `right_pillar`, `right_repeater`).
///
/// The canonical order is used for tile order, readiness scans, digit shortcuts (`1`–`6`), camera
/// availability and missing-camera lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CameraId {
    /// `front`
    Front,
    /// `back`
    Back,
    /// `left_pillar`
    LeftPillar,
    /// `left_repeater`
    LeftRepeater,
    /// `right_pillar`
    RightPillar,
    /// `right_repeater`
    RightRepeater,
}

impl CameraId {
    /// All cameras in canonical order.
    pub const ALL: [CameraId; 6] = [
        CameraId::Front,
        CameraId::Back,
        CameraId::LeftPillar,
        CameraId::LeftRepeater,
        CameraId::RightPillar,
        CameraId::RightRepeater,
    ];

    /// The order in which the six windows fill with video after a folder loads (`DESIGN.md` §7,
    /// mount order with the front first; same as the web's `revealOrder`).
    pub const REVEAL_ORDER: [CameraId; 6] = [
        CameraId::Front,
        CameraId::LeftPillar,
        CameraId::RightPillar,
        CameraId::LeftRepeater,
        CameraId::RightRepeater,
        CameraId::Back,
    ];

    /// Position in the canonical order (0–5).
    pub const fn index(self) -> usize {
        match self {
            CameraId::Front => 0,
            CameraId::Back => 1,
            CameraId::LeftPillar => 2,
            CameraId::LeftRepeater => 3,
            CameraId::RightPillar => 4,
            CameraId::RightRepeater => 5,
        }
    }

    /// The file-name id, e.g. `left_pillar`.
    pub const fn id(self) -> &'static str {
        match self {
            CameraId::Front => "front",
            CameraId::Back => "back",
            CameraId::LeftPillar => "left_pillar",
            CameraId::LeftRepeater => "left_repeater",
            CameraId::RightPillar => "right_pillar",
            CameraId::RightRepeater => "right_repeater",
        }
    }

    /// Parses a file-name id (case-sensitive).
    pub fn from_id(id: &str) -> Option<CameraId> {
        CameraId::ALL.into_iter().find(|camera| camera.id() == id)
    }

    /// The camera of digit shortcut `digit` (`1` = front … `6` = right repeater).
    pub fn from_digit(digit: u8) -> Option<CameraId> {
        CameraId::ALL
            .get(usize::from(digit).checked_sub(1)?)
            .copied()
    }

    /// The Japanese camera name, e.g. `左ピラー`.
    pub const fn label(self) -> &'static str {
        crate::copy::camera_label(self)
    }
}

/// The folder a clip came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClipCategory {
    /// `RecentClips`: rolling dashcam recording.
    Recent,
    /// `SavedClips`: clips the driver saved.
    Saved,
    /// `SentryClips`: Sentry Mode events.
    Sentry,
}

impl ClipCategory {
    /// All categories in library order (`recent`, `saved`, `sentry`).
    pub const ALL: [ClipCategory; 3] = [
        ClipCategory::Recent,
        ClipCategory::Saved,
        ClipCategory::Sentry,
    ];

    /// The id used in event ids and the search haystack, e.g. `sentry`.
    pub const fn id(self) -> &'static str {
        match self {
            ClipCategory::Recent => "recent",
            ClipCategory::Saved => "saved",
            ClipCategory::Sentry => "sentry",
        }
    }

    /// Parses an id.
    pub fn from_id(id: &str) -> Option<ClipCategory> {
        ClipCategory::ALL
            .into_iter()
            .find(|category| category.id() == id)
    }

    /// The TeslaCam folder name, e.g. `SentryClips`.
    pub const fn folder_name(self) -> &'static str {
        match self {
            ClipCategory::Recent => "RecentClips",
            ClipCategory::Saved => "SavedClips",
            ClipCategory::Sentry => "SentryClips",
        }
    }

    /// The Japanese category name, e.g. `セントリー`.
    pub const fn label(self) -> &'static str {
        crate::copy::category_label(self)
    }
}

/// A fixed-size map with one value per camera, indexed by [`CameraId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CameraMap<T> {
    values: [T; 6],
}

impl<T> CameraMap<T> {
    /// Builds a map by calling `value` for every camera in canonical order.
    pub fn from_fn(mut value: impl FnMut(CameraId) -> T) -> Self {
        CameraMap {
            values: CameraId::ALL.map(&mut value),
        }
    }

    /// The value of `camera`.
    pub fn get(&self, camera: CameraId) -> &T {
        &self.values[camera.index()]
    }

    /// The value of `camera`, mutably.
    pub fn get_mut(&mut self, camera: CameraId) -> &mut T {
        &mut self.values[camera.index()]
    }

    /// Iterates `(camera, value)` in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = (CameraId, &T)> {
        CameraId::ALL.into_iter().zip(self.values.iter())
    }

    /// Applies `transform` to every value.
    pub fn map<U>(&self, mut transform: impl FnMut(CameraId, &T) -> U) -> CameraMap<U> {
        CameraMap::from_fn(|camera| transform(camera, &self[camera]))
    }

    /// The cameras whose value satisfies `predicate`, in canonical order.
    pub fn cameras_where(&self, mut predicate: impl FnMut(&T) -> bool) -> Vec<CameraId> {
        self.iter()
            .filter(|(_, value)| predicate(value))
            .map(|(camera, _)| camera)
            .collect()
    }
}

impl<T> CameraMap<Option<T>> {
    /// The cameras that have a value, in canonical order.
    pub fn present(&self) -> Vec<CameraId> {
        self.cameras_where(Option::is_some)
    }
}

impl<T> IntoIterator for CameraMap<T> {
    type Item = (CameraId, T);
    type IntoIter = std::iter::Zip<std::array::IntoIter<CameraId, 6>, std::array::IntoIter<T, 6>>;

    /// `(camera, value)` pairs in canonical order.
    fn into_iter(self) -> Self::IntoIter {
        CameraId::ALL.into_iter().zip(self.values)
    }
}

impl<T> Index<CameraId> for CameraMap<T> {
    type Output = T;

    fn index(&self, camera: CameraId) -> &T {
        self.get(camera)
    }
}

impl<T> IndexMut<CameraId> for CameraMap<T> {
    fn index_mut(&mut self, camera: CameraId) -> &mut T {
        self.get_mut(camera)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_order_and_ids() {
        let ids = CameraId::ALL.map(CameraId::id);
        assert_eq!(
            ids,
            [
                "front",
                "back",
                "left_pillar",
                "left_repeater",
                "right_pillar",
                "right_repeater"
            ]
        );
        for (index, camera) in CameraId::ALL.into_iter().enumerate() {
            assert_eq!(camera.index(), index);
            assert_eq!(CameraId::from_id(camera.id()), Some(camera));
        }
        assert_eq!(CameraId::from_id("Front"), None);
        assert_eq!(CameraId::from_id("cabin"), None);
    }

    #[test]
    fn the_reveal_order_covers_every_camera_once() {
        let mut order = CameraId::REVEAL_ORDER.to_vec();
        assert_eq!(order[0], CameraId::Front);
        order.sort();
        assert_eq!(order, CameraId::ALL.to_vec());
    }

    #[test]
    fn digits_follow_the_canonical_order() {
        assert_eq!(CameraId::from_digit(1), Some(CameraId::Front));
        assert_eq!(CameraId::from_digit(4), Some(CameraId::LeftRepeater));
        assert_eq!(CameraId::from_digit(6), Some(CameraId::RightRepeater));
        assert_eq!(CameraId::from_digit(0), None);
        assert_eq!(CameraId::from_digit(7), None);
    }

    #[test]
    fn categories_round_trip() {
        for category in ClipCategory::ALL {
            assert_eq!(ClipCategory::from_id(category.id()), Some(category));
        }
        assert_eq!(ClipCategory::Sentry.folder_name(), "SentryClips");
        assert_eq!(ClipCategory::from_id("encrypted"), None);
    }

    #[test]
    fn camera_map_indexes_by_camera() {
        let mut map: CameraMap<Option<u32>> = CameraMap::default();
        map[CameraId::Back] = Some(2);
        map[CameraId::RightRepeater] = Some(6);
        assert_eq!(map.present(), vec![CameraId::Back, CameraId::RightRepeater]);
        let doubled = map.map(|_, value| value.map(|value| value * 2));
        assert_eq!(doubled[CameraId::Back], Some(4));
        assert_eq!(
            CameraMap::from_fn(CameraId::index)[CameraId::RightPillar],
            4
        );
    }

    #[test]
    fn camera_map_accessors_and_owned_iteration() {
        let mut map = CameraMap::from_fn(|camera| camera.index() * 10);
        *map.get_mut(CameraId::Back) = 7;
        assert_eq!(*map.get(CameraId::Back), 7);
        assert_eq!(*map.get(CameraId::LeftRepeater), 30);
        let owned: Vec<(CameraId, String)> =
            map.map(|_, value| value.to_string()).into_iter().collect();
        assert_eq!(owned.len(), 6);
        assert_eq!(owned[0], (CameraId::Front, "0".to_owned()));
        assert_eq!(owned[5], (CameraId::RightRepeater, "50".to_owned()));
    }
}
