use std::str::FromStr;

use crate::lib::track_observations::TrackObservation;
use crate::lib::zones::VirtualLineDirection;
use crate::settings::{RoadLanesSettings, ViolationsSettings};
use crate::violations::event::ViolationEvent;
use crate::violations::rules::no_parking::NoParkingRule;
use crate::violations::rules::no_stopping::NoStoppingRule;
use crate::violations::rules::solid_line::SolidLineRule;
use crate::violations::rules::speeding::SpeedingRule;
use crate::violations::rules::wrong_way::WrongWayRule;
use crate::violations::rules::ViolationRule;

#[derive(Debug, Clone, Copy)]
pub struct RuleRuntimeConfig {
    pub state_ttl_sec: f32,
    pub cleanup_interval_sec: f32,
}

pub struct ViolationCoordinator {
    rules: Vec<Box<dyn ViolationRule>>,
}

impl ViolationCoordinator {
    /// Creates the configured violation rules for each road lane.
    /// Returns a coordinator with no rules when violations are disabled.
    ///
    /// Each rule receives its own configuration and track state, while
    /// runtime cleanup settings are copied from the global configuration.
    ///
    /// A wrong-way rule is created only when the lane has a virtual line
    /// with a recognized direction and non-degenerate geometry.
    ///
    /// Rules are built from the supplied settings at construction time;
    /// later changes to those settings are not applied automatically.
    pub fn new(config: ViolationsSettings, road_lanes: &[RoadLanesSettings]) -> Self {
        let mut rules: Vec<Box<dyn ViolationRule>> = Vec::new();

        if config.enabled {
            let runtime = RuleRuntimeConfig {
                state_ttl_sec: config.state_ttl_sec,
                cleanup_interval_sec: config.cleanup_interval_sec,
            };

            for lane in road_lanes {
                let zone_id = lane.zone_id();

                if let Some(rule) = &lane.rules.solid_line {
                    rules.push(Box::new(SolidLineRule::from_embedded(
                        zone_id.clone(),
                        rule.clone(),
                        runtime,
                    )));
                }

                if let Some(rule) = &lane.rules.speeding {
                    rules.push(Box::new(SpeedingRule::from_embedded(
                        zone_id.clone(),
                        lane.geometry.clone(),
                        rule.clone(),
                        runtime,
                    )));
                }

                if let Some(rule) = &lane.rules.no_stopping {
                    rules.push(Box::new(NoStoppingRule::from_embedded(
                        zone_id.clone(),
                        lane.geometry.clone(),
                        rule.clone(),
                        runtime,
                    )));
                }

                if let Some(rule) = &lane.rules.no_parking {
                    rules.push(Box::new(NoParkingRule::from_embedded(
                        zone_id.clone(),
                        lane.geometry.clone(),
                        rule.clone(),
                        runtime,
                    )));
                }

                if let Some(rule) = &lane.rules.wrong_way {
                    if let Some(virtual_line) = &lane.virtual_line {
                        if let Ok(direction) =
                            VirtualLineDirection::from_str(&virtual_line.direction)
                        {
                            if let Some(allowed_direction) =
                                allowed_direction_from_virtual_line(
                                    virtual_line.geometry,
                                    direction,
                                )
                            {
                                rules.push(Box::new(WrongWayRule::from_embedded(
                                    zone_id.clone(),
                                    lane.geometry.clone(),
                                    allowed_direction,
                                    rule.min_direction_samples,
                                    runtime,
                                )));
                            }
                        }
                    }
                }
            }
        }

        Self { rules }
    }

    /// Passes the observation to every configured rule and collects
    /// their events in rule iteration order.
    /// Each rule determines whether the observation applies to it.
    pub fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let mut out = Vec::new();

        for rule in self.rules.iter_mut() {
            out.extend(rule.process(obs));
        }

        out
    }
}

/// Computes the allowed unit direction perpendicular to the virtual line.
/// Outbound uses the normal (-dy, dx), while inbound uses its opposite.
///
/// The result depends on endpoint order: swapping the endpoints reverses
/// the returned direction for the same inbound/outbound setting.
/// Returns None if the computed line length is non-finite or too small.
fn allowed_direction_from_virtual_line(
    geometry: [[i32; 2]; 2],
    direction: VirtualLineDirection,
) -> Option<[f32; 2]> {
    let ax = geometry[0][0] as f32;
    let ay = geometry[0][1] as f32;
    let bx = geometry[1][0] as f32;
    let by = geometry[1][1] as f32;

    let dx = bx - ax;
    let dy = by - ay;

    let length = (dx * dx + dy * dy).sqrt();
    if !length.is_finite() || length <= f32::EPSILON {
        return None;
    }

    let left_normal = [-dy / length, dx / length];

    let vector = match direction {
        VirtualLineDirection::Inbound => [-left_normal[0], -left_normal[1]],
        VirtualLineDirection::Outbound => left_normal,
    };

    Some(vector)
}