use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, mpsc};

use crate::lib::cv::RawFrame;

pub struct ThreadedFrame {
    pub frame: RawFrame,
    /// Seconds since capture start. Media time (frame index / fps) for files,
    /// wall clock for live sources. f64 so that the interval between two frames
    /// stays exact after days of uptime; f32 loses ~10 ms per step past ~1e5 s.
    pub timestamp: f64,
}

/// Hands frames from the capture thread to the detection thread.
///
/// A file is consumed losslessly: the capture thread waits for the detector, so
/// every N-th frame is processed and report mode stays deterministic. A live
/// source never waits: the newest frame replaces the one the detector has not
/// taken yet. A detector that falls behind then sees real, longer intervals
/// between the frames it processes instead of a growing backlog of stale ones,
/// the decoder is never blocked on the pipe, and latency stays bounded by one
/// frame.
pub fn frame_channel(live: bool) -> (FrameSender, FrameReceiver) {
    if live {
        let slot = Arc::new(LatestFrameSlot::default());
        (
            FrameSender::Latest(slot.clone()),
            FrameReceiver::Latest(slot),
        )
    } else {
        let (tx, rx) = mpsc::sync_channel(0);
        (FrameSender::Lossless(tx), FrameReceiver::Lossless(rx))
    }
}

pub enum FrameSender {
    Lossless(SyncSender<ThreadedFrame>),
    Latest(Arc<LatestFrameSlot>),
}

pub enum FrameReceiver {
    Lossless(Receiver<ThreadedFrame>),
    Latest(Arc<LatestFrameSlot>),
}

impl FrameSender {
    /// Hands a frame over. Blocks until the detector takes it for a file; for a
    /// live source returns at once, replacing a frame the detector has not taken
    /// yet. `Err` means the receiving side is gone.
    pub fn send(&self, frame: ThreadedFrame) -> Result<(), ThreadedFrame> {
        match self {
            FrameSender::Lossless(tx) => tx.send(frame).map_err(|e| e.0),
            FrameSender::Latest(slot) => slot.push(frame),
        }
    }

    /// Frames replaced before the detector took them. Always 0 for a file.
    pub fn dropped(&self) -> u64 {
        match self {
            FrameSender::Lossless(_) => 0,
            FrameSender::Latest(slot) => slot.dropped(),
        }
    }
}

impl Drop for FrameSender {
    fn drop(&mut self) {
        if let FrameSender::Latest(slot) = self {
            slot.close();
        }
    }
}

impl Drop for FrameReceiver {
    fn drop(&mut self) {
        if let FrameReceiver::Latest(slot) = self {
            // Lets the capture thread notice that nobody will take frames any more
            slot.close();
        }
    }
}

impl FrameReceiver {
    /// Next frame to process. `None` once the sender is gone and nothing is left.
    pub fn recv(&self) -> Option<ThreadedFrame> {
        match self {
            FrameReceiver::Lossless(rx) => rx.recv().ok(),
            FrameReceiver::Latest(slot) => slot.take(),
        }
    }

    /// Frames replaced before the detector took them, so far. Always 0 for a file.
    pub fn dropped(&self) -> u64 {
        match self {
            FrameReceiver::Lossless(_) => 0,
            FrameReceiver::Latest(slot) => slot.dropped(),
        }
    }
}

/// Single-frame mailbox: the newest frame wins.
#[derive(Default)]
pub struct LatestFrameSlot {
    state: Mutex<SlotState>,
    ready: Condvar,
    dropped: AtomicU64,
}

#[derive(Default)]
struct SlotState {
    frame: Option<ThreadedFrame>,
    closed: bool,
}

impl LatestFrameSlot {
    fn push(&self, frame: ThreadedFrame) -> Result<(), ThreadedFrame> {
        let mut state = self.state.lock().expect("Frame slot is poisoned [Mutex]");
        if state.closed {
            return Err(frame);
        }
        if state.frame.replace(frame).is_some() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        self.ready.notify_one();
        Ok(())
    }

    fn take(&self) -> Option<ThreadedFrame> {
        let mut state = self.state.lock().expect("Frame slot is poisoned [Mutex]");
        loop {
            if let Some(frame) = state.frame.take() {
                return Some(frame);
            }
            if state.closed {
                return None;
            }
            state = self
                .ready
                .wait(state)
                .expect("Frame slot is poisoned [Mutex]");
        }
    }

    fn close(&self) {
        let mut state = self.state.lock().expect("Frame slot is poisoned [Mutex]");
        state.closed = true;
        self.ready.notify_all();
    }

    fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn frame(ts: f64) -> ThreadedFrame {
        ThreadedFrame {
            frame: RawFrame {
                data: vec![],
                width: 0,
                height: 0,
            },
            timestamp: ts,
        }
    }

    #[test]
    fn latest_slot_keeps_only_the_newest_frame() {
        let (tx, rx) = frame_channel(true);
        assert!(tx.send(frame(1.0)).is_ok());
        assert!(tx.send(frame(2.0)).is_ok());
        assert!(tx.send(frame(3.0)).is_ok());
        assert_eq!(rx.recv().unwrap().timestamp, 3.0);
        assert_eq!(rx.dropped(), 2);
        assert_eq!(tx.dropped(), 2);
    }

    #[test]
    fn latest_slot_waits_for_a_frame_and_ends_on_close() {
        let (tx, rx) = frame_channel(true);
        let producer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            assert!(tx.send(frame(1.0)).is_ok());
            thread::sleep(Duration::from_millis(20));
            // tx dropped here: the receiver must wake up and stop
        });
        assert_eq!(rx.recv().unwrap().timestamp, 1.0);
        assert!(rx.recv().is_none());
        producer.join().unwrap();
        assert_eq!(rx.dropped(), 0);
    }

    #[test]
    fn latest_slot_refuses_frames_after_receiver_is_gone() {
        let (tx, rx) = frame_channel(true);
        drop(rx);
        assert!(tx.send(frame(1.0)).is_err());
    }

    #[test]
    fn lossless_channel_delivers_every_frame_in_order() {
        let (tx, rx) = frame_channel(false);
        let producer = thread::spawn(move || {
            for i in 0..5 {
                assert!(tx.send(frame(i as f64)).is_ok());
            }
        });
        let mut seen = vec![];
        while let Some(f) = rx.recv() {
            seen.push(f.timestamp);
        }
        producer.join().unwrap();
        assert_eq!(seen, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!(rx.dropped(), 0);
    }
}
