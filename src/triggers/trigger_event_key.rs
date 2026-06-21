use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TriggerEventKey(u64);

impl TriggerEventKey {
    pub fn new<T: Hash>(event: &T) -> Self {
        let mut hasher = DefaultHasher::new();
        event.hash(&mut hasher);
        Self(hasher.finish())
    }
}
