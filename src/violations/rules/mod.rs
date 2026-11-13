pub mod speeding;
pub mod state;
pub mod stationary;
pub mod wrong_way;

use crate::lib::track_observations::TrackObservation;
use crate::violations::event::ViolationEvent;

/// A violation rule evaluates the observations of one frame and returns the
/// events that frame produced. Rules keep their own per-track state and are
/// driven only through this method: the coordinator decides which rules exist
/// and feeds every rule the same frame batch.
pub trait ViolationRule: Send {
    fn process_frame(&mut self, observations: &[TrackObservation]) -> Vec<ViolationEvent>;
}
