//! Shared per-track state storage for violation rules.
//!
//! Every rule that keeps per-track state needs the same bookkeeping: expire
//! entries that have not been seen for `state_ttl_sec`, and run the sweep at
//! most once per `cleanup_interval_sec`. This store is the one implementation
//! of that bookkeeping; the rules only carry their domain state.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use uuid::Uuid;

/// What the store needs to know about a rule's state value: when the track
/// was last seen, in source time.
pub trait TrackStateEntry {
    fn last_seen_at(&self) -> f32;
}

pub struct RuleStateStore<V> {
    entries: HashMap<Uuid, V>,
    state_ttl_sec: f32,
    cleanup_interval_sec: f32,
    next_cleanup_at: f32,
}

impl<V: TrackStateEntry> RuleStateStore<V> {
    pub fn new(state_ttl_sec: f32, cleanup_interval_sec: f32) -> Self {
        RuleStateStore {
            entries: HashMap::new(),
            state_ttl_sec,
            cleanup_interval_sec,
            next_cleanup_at: 0.0,
        }
    }

    /// Removes expired track state when the next cleanup is due. Track age is
    /// measured from its last observation in source time; entries with a
    /// negative age are also discarded.
    pub fn cleanup_if_needed(&mut self, now: f32) {
        if now < self.next_cleanup_at {
            return;
        }

        let ttl = self.state_ttl_sec;
        self.entries.retain(|_, state| {
            let age = now - state.last_seen_at();
            age >= 0.0 && age <= ttl
        });

        self.next_cleanup_at = now + self.cleanup_interval_sec;
    }

    /// Whether the track's entry is stale at `now`: never seen since the TTL
    /// passed, or seen with a negative age.
    pub fn expired(&self, track_id: &Uuid, now: f32) -> bool {
        self.entries.get(track_id).is_some_and(|state| {
            let age = now - state.last_seen_at();
            age < 0.0 || age > self.state_ttl_sec
        })
    }

    /// Removes the track's entry when it is stale at `now`, so a single track
    /// expires between full cleanup passes.
    pub fn remove_expired(&mut self, track_id: &Uuid, now: f32) {
        if self.expired(track_id, now) {
            self.entries.remove(track_id);
        }
    }

    pub fn get(&self, track_id: &Uuid) -> Option<&V> {
        self.entries.get(track_id)
    }

    pub fn get_mut(&mut self, track_id: &Uuid) -> Option<&mut V> {
        self.entries.get_mut(track_id)
    }

    pub fn remove(&mut self, track_id: &Uuid) -> Option<V> {
        self.entries.remove(track_id)
    }

    pub fn entry(&mut self, track_id: Uuid) -> Entry<'_, Uuid, V> {
        self.entries.entry(track_id)
    }

    /// Test hook: how many tracks this store currently remembers.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct State {
        last_seen_at: f32,
    }

    impl TrackStateEntry for State {
        fn last_seen_at(&self) -> f32 {
            self.last_seen_at
        }
    }

    fn state(last_seen_at: f32) -> State {
        State { last_seen_at }
    }

    #[test]
    fn cleanup_runs_at_most_once_per_interval_and_expires_by_age() {
        let mut store = RuleStateStore::new(7.0, 1.0);
        let track = Uuid::new_v4();
        store.entry(track).or_insert(state(0.0));

        // The first call always sweeps: the entry is 0.5 s old, kept.
        store.cleanup_if_needed(0.5);
        assert_eq!(store.len(), 1);

        // The next sweep is due at 1.5 s: this call is skipped.
        store.cleanup_if_needed(1.0);
        assert_eq!(store.len(), 1);

        // At 7.1 s a sweep runs and the entry is older than the 7.0 s TTL.
        store.cleanup_if_needed(7.1);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn negative_age_is_stale() {
        let mut store = RuleStateStore::new(7.0, 1.0);
        let track = Uuid::new_v4();
        store.entry(track).or_insert(state(10.0));
        assert!(store.expired(&track, 5.0));
        store.remove_expired(&track, 5.0);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn remove_expired_keeps_fresh_entries() {
        let mut store = RuleStateStore::new(7.0, 1.0);
        let track = Uuid::new_v4();
        store.entry(track).or_insert(state(0.0));
        store.remove_expired(&track, 3.0);
        assert_eq!(store.len(), 1);
    }
}
