// Based on https://github.com/LdDl/mjpeg-rs/blob/master/src/mjpeg_streaming/broadcaster.rs

use std::{
    pin::Pin,
    sync::{Mutex, mpsc::Receiver as STDReceiver},
    task::{Context, Poll},
    thread,
    time::{Duration, Instant},
};

use actix_web::{Error, web};

use futures::Stream;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tracing::{debug, info, warn};

use crate::lib::logging;

/// How many frames a client may be behind before the newest one is dropped.
/// Small on purpose: whatever waits here is already out of date, and a client
/// that catches up should see live video rather than work through a backlog.
const CLIENT_BUFFER_SIZE: usize = 5;

/// A client that has taken nothing for this long is gone in every way but the
/// socket - a half-open connection, or a machine that went away without closing
/// it. Generous on purpose: cutting a connection is what froze the picture in
/// the first place, while holding a dead one costs a few buffered frames, and
/// the kernel gives up on an unacknowledged socket by itself soon enough.
const STALE_CLIENT_AFTER: Duration = Duration::from_secs(300);

struct ClientState {
    sender: Sender<web::Bytes>,
    /// When this client last took a frame, which is what tells a connection
    /// that is merely slow apart from one that is dead
    last_taken: Instant,
    /// Frames this client was too slow to take
    dropped: u64,
}

pub struct Broadcaster {
    clients: Vec<ClientState>,
}

impl Default for Broadcaster {
    fn default() -> Self {
        Broadcaster {
            clients: Vec::new(),
        }
    }
}

impl Broadcaster {
    pub fn add_client(&mut self) -> Client {
        let (tx, rx) = channel(CLIENT_BUFFER_SIZE);
        self.clients.push(ClientState {
            sender: tx,
            last_taken: Instant::now(),
            dropped: 0,
        });
        info!(
            scope = logging::SCOPE_MJPEG,
            clients = self.clients.len(),
            "A viewer opened the stream"
        );
        Client(rx)
    }

    pub fn make_message_block(buffer: &[u8]) -> Vec<u8> {
        let bfu8 = buffer;
        let header = format!(
            "--boundarydonotcross\r\nContent-Length:{}\r\nContent-Type:image/jpeg\r\n\r\n",
            bfu8.len()
        );
        let mut msg = Vec::with_capacity(header.len() + bfu8.len());
        msg.extend_from_slice(header.as_bytes());
        msg.extend_from_slice(bfu8);
        msg
    }

    /// A slow client loses the frame, never the connection. Live video is worth
    /// more fresh than complete, and the stream is a single response body: once
    /// it ends the browser keeps the last frame on screen and never asks for
    /// another, so dropping a client that is merely behind freezes the picture
    /// until the page is reloaded.
    fn send_image(&mut self, msg: Vec<u8>) {
        if self.clients.is_empty() {
            return;
        }
        let bytes = web::Bytes::from(msg);
        let now = Instant::now();
        let mut left = 0;
        self.clients
            .retain_mut(|client| match client.sender.try_send(bytes.clone()) {
                Ok(()) => {
                    client.last_taken = now;
                    true
                }
                Err(TrySendError::Full(_)) => {
                    client.dropped += 1;
                    let idle = now.duration_since(client.last_taken);
                    if idle < STALE_CLIENT_AFTER {
                        return true;
                    }
                    warn!(
                        scope = logging::SCOPE_MJPEG,
                        idle_seconds = idle.as_secs(),
                        dropped_frames = client.dropped,
                        "Closed a stream nobody has been reading"
                    );
                    left += 1;
                    false
                }
                Err(TrySendError::Closed(_)) => {
                    debug!(
                        scope = logging::SCOPE_MJPEG,
                        dropped_frames = client.dropped,
                        "A viewer closed the stream"
                    );
                    left += 1;
                    false
                }
            });
        if left > 0 {
            info!(
                scope = logging::SCOPE_MJPEG,
                clients = self.clients.len(),
                "Viewers left the stream"
            );
        }
    }

    pub fn spawn_reciever(_self: web::Data<Mutex<Self>>, rx_frames_data: STDReceiver<Vec<u8>>) {
        thread::spawn(move || {
            for received in rx_frames_data {
                let msg = Broadcaster::make_message_block(&received);
                _self.lock().unwrap().send_image(msg);
            }
        });
    }
}

pub struct Client(Receiver<web::Bytes>);

impl Stream for Client {
    type Item = Result<web::Bytes, Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.0).poll_recv(cx) {
            Poll::Ready(Some(v)) => Poll::Ready(Some(Ok(v))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Vec<u8> {
        Broadcaster::make_message_block(&[0xFF, 0xD8, 0xFF, 0xD9])
    }

    #[test]
    fn a_client_that_stopped_reading_keeps_its_stream() {
        let mut broadcaster = Broadcaster::default();
        let client = broadcaster.add_client();
        // Far more than the buffer holds, and nothing is being read
        for _ in 0..CLIENT_BUFFER_SIZE + 100 {
            broadcaster.send_image(frame());
        }
        assert_eq!(broadcaster.clients.len(), 1, "the stream was cut short");
        assert!(broadcaster.clients[0].dropped >= 100);
        drop(client);
    }

    #[test]
    fn a_client_that_reads_again_gets_frames_again() {
        let mut broadcaster = Broadcaster::default();
        let mut client = broadcaster.add_client();
        for _ in 0..CLIENT_BUFFER_SIZE + 100 {
            broadcaster.send_image(frame());
        }
        // The viewer comes back: it works through what is buffered, and the
        // next frame the app produces reaches it
        while client.0.try_recv().is_ok() {}
        broadcaster.send_image(frame());
        assert!(client.0.try_recv().is_ok(), "no frames after catching up");
    }

    #[test]
    fn a_client_that_went_away_is_dropped() {
        let mut broadcaster = Broadcaster::default();
        let client = broadcaster.add_client();
        drop(client);
        broadcaster.send_image(frame());
        assert!(broadcaster.clients.is_empty());
    }

    #[test]
    fn a_client_that_reads_nothing_at_all_is_dropped_eventually() {
        let mut broadcaster = Broadcaster::default();
        let client = broadcaster.add_client();
        for _ in 0..CLIENT_BUFFER_SIZE + 1 {
            broadcaster.send_image(frame());
        }
        assert_eq!(broadcaster.clients.len(), 1);
        // Rather than waiting a minute for it
        broadcaster.clients[0].last_taken -= STALE_CLIENT_AFTER;
        broadcaster.send_image(frame());
        assert!(broadcaster.clients.is_empty());
        drop(client);
    }
}
