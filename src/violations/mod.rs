pub mod coordinator;
pub mod event;
pub mod rules;
pub mod sink;

use crate::lib::track_observations::{TrackObservationRx, TrackObservations};
use crate::settings::{RoadLanesSettings, ViolationsSettings};
use self::coordinator::ViolationCoordinator;
use self::sink::{ConsoleSink, EventSink};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{info, warn};

/// Subscribes to track observations and starts an asynchronous task
/// that evaluates violation rules and publishes events to the console sink.
///
/// The coordinator is created once from the supplied configuration;
/// subsequent changes to zone or rule settings are not applied automatically.
///
/// Logs a warning if the receiver falls behind and observations are skipped.
/// Exits when the observation channel is closed and all buffered messages
/// have been consumed. Returns the spawned task's join handle.
pub fn spawn_violation_observer(
    tx: &TrackObservations,
    config: ViolationsSettings,
    road_lanes: Vec<RoadLanesSettings>,
) -> JoinHandle<()> {
    let mut rx: TrackObservationRx = tx.subscribe();
    let sink = ConsoleSink;

    tokio::spawn(async move {
        let mut coordinator = ViolationCoordinator::new(config, &road_lanes);

        loop {
            match rx.recv().await {
                Ok(obs) => {
                    let events = coordinator.process(&obs);
                    for event in events {
                        sink.publish(&event);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(
                        target: "violation_event",
                        skipped,
                        "Violation observer lagged behind"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!(
                        target: "violation_event",
                        "Track observation channel closed"
                    );
                    break;
                }
            }
        }
    })
}