//! Least-recently-used bookkeeping for the thumbnail cache: which keys to evict when a new one is
//! used. Pure; the cache that owns the images lives in `ui::thumbnails`.

use std::collections::VecDeque;

/// Keys in least-recently-used order with a fixed capacity.
#[derive(Debug, Clone)]
pub struct Lru<K> {
    capacity: usize,
    order: VecDeque<K>,
}

impl<K: PartialEq + Clone> Lru<K> {
    /// An empty set holding at most `capacity` keys (at least 1).
    pub fn new(capacity: usize) -> Lru<K> {
        Lru {
            capacity: capacity.max(1),
            order: VecDeque::new(),
        }
    }

    /// Marks `key` as the most recently used and returns the keys that no longer fit, oldest first.
    pub fn touch(&mut self, key: K) -> Vec<K> {
        if let Some(index) = self.order.iter().position(|candidate| *candidate == key) {
            self.order.remove(index);
        }
        self.order.push_back(key);
        let excess = self.order.len().saturating_sub(self.capacity);
        self.order.drain(..excess).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_the_least_recently_used_key() {
        let mut lru = Lru::new(2);
        assert!(lru.touch("a").is_empty());
        assert!(lru.touch("b").is_empty());
        // Using "a" again makes "b" the oldest.
        assert!(lru.touch("a").is_empty());
        assert_eq!(lru.touch("c"), ["b"]);
        assert_eq!(lru.touch("d"), ["a"]);
    }

    #[test]
    fn a_zero_capacity_still_holds_one_key() {
        let mut lru = Lru::new(0);
        assert_eq!(lru.touch(1), Vec::<i32>::new());
        assert_eq!(lru.touch(2), [1]);
        assert_eq!(lru.touch(2), Vec::<i32>::new());
    }
}
