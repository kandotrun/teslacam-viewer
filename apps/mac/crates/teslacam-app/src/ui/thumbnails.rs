//! A bounded image cache for event-row thumbnails (`thumb.png`): rows load their thumbnail when the
//! virtual list renders them, and the least recently used images are released.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use gpui_kit::{
    App, Context, ImageCache, ImageCacheError, ImageCacheItem, RenderImage, Resource, Window,
};

use crate::presentation::lru::Lru;

/// How many decoded thumbnails stay in memory.
pub const THUMBNAIL_CAPACITY: usize = 160;

/// The cache entity (used with `image_cache(entity)` around the event list).
pub struct ThumbnailCache {
    items: HashMap<u64, ImageCacheItem>,
    lru: Lru<u64>,
}

impl ThumbnailCache {
    /// An empty cache.
    pub fn new(_cx: &mut Context<Self>) -> ThumbnailCache {
        ThumbnailCache {
            items: HashMap::new(),
            lru: Lru::new(THUMBNAIL_CAPACITY),
        }
    }
}

fn key(resource: &Resource) -> u64 {
    let mut hasher = DefaultHasher::new();
    resource.hash(&mut hasher);
    hasher.finish()
}

impl ImageCache for ThumbnailCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let key = key(resource);
        for evicted in self.lru.touch(key) {
            if let Some(Some(Ok(image))) = self.items.remove(&evicted).map(|item| item.get()) {
                cx.drop_image(image, Some(window));
            }
        }
        self.items
            .entry(key)
            .or_insert_with(|| ImageCacheItem::new(resource, cx))
            .use_image(window)
    }
}
