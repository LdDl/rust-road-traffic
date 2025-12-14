// Based on https://github.com/LdDl/mjpeg-rs/blob/master/src/mjpeg_streaming/broadcaster.rs

use opencv::core::Vector;

use std::{
    thread,
    sync::{
        Mutex,
        mpsc::Receiver as STDReceiver
    },
    task::{Context, Poll},
    pin::Pin
};

use actix_web::{web, Error};

use futures::Stream;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::mpsc::error::TrySendError;

/// Channel buffer size for each client.
/// Larger buffer allows clients to handle temporary slowdowns without being disconnected.
const CLIENT_BUFFER_SIZE: usize = 5;

/// Number of consecutive send failures before removing a client.
/// This prevents dropping clients due to momentary network hiccups.
const MAX_CONSECUTIVE_FAILURES: u8 = 10;

struct ClientState {
    sender: Sender<web::Bytes>,
    consecutive_failures: u8,
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
            consecutive_failures: 0,
        });
        Client(rx)
    }

    pub fn make_message_block(buffer: &Vector<u8>) -> Vec<u8> {
        let bfu8 = buffer.as_ref();
        let header = format!(
            "--boundarydonotcross\r\nContent-Length:{}\r\nContent-Type:image/jpeg\r\n\r\n",
            bfu8.len()
        );
        let mut msg = Vec::with_capacity(header.len() + bfu8.len());
        msg.extend_from_slice(header.as_bytes());
        msg.extend_from_slice(bfu8);
        msg
    }

    fn send_image(&mut self, msg: &[u8]) {
        if self.clients.is_empty() {
            return;
        }
        let bytes = web::Bytes::from(msg.to_vec());
        // Update failure counts and remove clients that exceeded max failures
        self.clients.retain_mut(|client| {
            match client.sender.try_send(bytes.clone()) {
                Ok(()) => {
                    client.consecutive_failures = 0;
                    true
                }
                Err(TrySendError::Full(_)) => {
                    // Channel full - client is slow, increment failure count
                    client.consecutive_failures += 1;
                    if client.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        println!("[MJPEG] Removing slow client after {} consecutive failures", MAX_CONSECUTIVE_FAILURES);
                        false
                    } else {
                        // Keep client, they might recover
                        true
                    }
                }
                Err(TrySendError::Closed(_)) => {
                    // Client disconnected
                    false
                }
            }
        });
    }

    pub fn spawn_reciever(_self: web::Data<Mutex<Self>>, rx_frames_data: STDReceiver<Vector<u8>>) {
        thread::spawn(move || {
            for received in rx_frames_data {
                let msg = Broadcaster::make_message_block(&received);
                _self.lock().unwrap().send_image(&msg);
            }
        });
    }
}

pub struct Client (
    Receiver<web::Bytes>
);

impl Stream for Client {
    type Item = Result<web::Bytes, Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.0).poll_recv(cx) {
            Poll::Ready(Some(v)) => Poll::Ready(Some(Ok(v))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending
        }
    }
}