pub mod coordinator;
pub mod event;
pub mod rules;
pub mod sink;
pub mod solid_line;

use self::coordinator::ViolationCoordinator;
use self::sink::{ConsoleSink, EventSink};
use crate::lib::data_storage::ThreadedDataStorage;
use crate::lib::track_observations::{TrackObservationRx, TrackObservations};
use crate::settings::ViolationsSettings;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{info, warn};

/// Subscribes to track observations and starts an asynchronous task
/// that evaluates the violation rules of every frame and publishes the
/// events to the console sink.
///
/// The coordinator reads the live zones the process runs (the same storage
/// the REST API mutates), so zone and rule changes made through the API take
/// effect on the next frame without a restart.
///
/// Logs a warning if the receiver falls behind and a whole frame batch is
/// skipped. Exits when the observation channel is closed and all buffered
/// messages have been consumed. Returns the spawned task's join handle.
pub fn spawn_violation_observer(
    tx: &TrackObservations,
    data_storage: ThreadedDataStorage,
    config: ViolationsSettings,
) -> JoinHandle<()> {
    let mut rx: TrackObservationRx = tx.subscribe();
    let sink = ConsoleSink;
    let zones = data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]")
        .zones
        .clone();

    tokio::spawn(async move {
        let mut coordinator = ViolationCoordinator::new(zones, config);

        loop {
            match rx.recv().await {
                Ok(batch) => {
                    let events = coordinator.process_frame(&batch);
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
