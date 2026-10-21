//! Plain input data for this crate.
//!
//! These types mirror the parts of `teslacam-core` the media layer needs (camera ids and the event's
//! segments on the wall-clock timeline). They are kept local and minimal so the app can map the core
//! types onto them with a few lines; once the core types are stable this module can re-export them.

use std::path::PathBuf;

/// One of the six TeslaCam cameras, in canonical order (spec §1.1).
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

    /// Position in canonical order (0 = front).
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The id used in TeslaCam file names and the shared contract (`front`, `left_pillar`, ...).
    pub const fn as_str(self) -> &'static str {
        match self {
            CameraId::Front => "front",
            CameraId::Back => "back",
            CameraId::LeftPillar => "left_pillar",
            CameraId::LeftRepeater => "left_repeater",
            CameraId::RightPillar => "right_pillar",
            CameraId::RightRepeater => "right_repeater",
        }
    }

    /// Parses a contract id such as `left_pillar`.
    pub fn parse(id: &str) -> Option<CameraId> {
        CameraId::ALL
            .into_iter()
            .find(|camera| camera.as_str() == id)
    }
}

/// A value per camera, indexed by [`CameraId`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CameraMap<T>([T; 6]);

impl<T> CameraMap<T> {
    /// Builds the map by calling `f` for every camera in canonical order.
    pub fn from_fn(mut f: impl FnMut(CameraId) -> T) -> Self {
        CameraMap(CameraId::ALL.map(&mut f))
    }

    /// The value for `camera`.
    pub fn get(&self, camera: CameraId) -> &T {
        &self.0[camera.index()]
    }

    /// The value for `camera`, mutably.
    pub fn get_mut(&mut self, camera: CameraId) -> &mut T {
        &mut self.0[camera.index()]
    }

    /// `(camera, value)` pairs in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = (CameraId, &T)> {
        CameraId::ALL.into_iter().zip(self.0.iter())
    }

    /// Maps every value, keeping the camera association.
    pub fn map<U>(&self, mut f: impl FnMut(CameraId, &T) -> U) -> CameraMap<U> {
        CameraMap::from_fn(|camera| f(camera, self.get(camera)))
    }
}

impl<T> std::ops::Index<CameraId> for CameraMap<T> {
    type Output = T;
    fn index(&self, camera: CameraId) -> &T {
        self.get(camera)
    }
}

impl<T> std::ops::IndexMut<CameraId> for CameraMap<T> {
    fn index_mut(&mut self, camera: CameraId) -> &mut T {
        self.get_mut(camera)
    }
}

/// One segment of an event: the clips that share one file-name timestamp.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SegmentInput {
    /// Wall-clock offset of this segment's timestamp from the event's first segment, in seconds
    /// (0 for the first segment).
    pub start_seconds: f64,
    /// The clip file per camera; `None` when the segment has no file for that camera.
    pub files: CameraMap<Option<PathBuf>>,
}

/// An event as the media layer sees it: its segments in ascending timestamp order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventInput {
    /// Segments in ascending order of `start_seconds`.
    pub segments: Vec<SegmentInput>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_ids_follow_canonical_order_and_contract_names() {
        let names: Vec<_> = CameraId::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            names,
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
            assert_eq!(CameraId::parse(camera.as_str()), Some(camera));
        }
        assert_eq!(CameraId::parse("rear"), None);
    }

    #[test]
    fn camera_map_indexes_by_camera() {
        let mut map = CameraMap::from_fn(|camera| camera.index() * 10);
        assert_eq!(map[CameraId::LeftRepeater], 30);
        map[CameraId::Back] = 7;
        assert_eq!(*map.get(CameraId::Back), 7);
        let doubled = map.map(|_, value| value * 2);
        assert_eq!(doubled[CameraId::RightRepeater], 100);
        assert_eq!(map.iter().count(), 6);
    }
}
