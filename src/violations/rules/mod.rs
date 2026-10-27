pub mod geometry;
pub mod no_parking;
pub mod no_stopping;
pub mod solid_line;
pub mod speeding;
pub mod wrong_way;

use crate::lib::track_observations::TrackObservation;
use crate::violations::event::ViolationEvent;

pub trait ViolationRule: Send {
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent>;
}