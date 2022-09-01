// Strictly taken from https://github.com/LdDl/mjpeg-rs/blob/master/src/mjpeg_streaming/broadcaster.rs

use opencv::{
    prelude::*,
    core::Vector,
};

use std::{
    thread,
    sync::{
        Mutex,
        mpsc::{
            Receiver as STDReceiver
        }
    },
    task::{
        Context,
        Poll
    },
    pin::Pin
};

use actix_web::{
    web,
    Error
};

use futures::Stream;
use tokio::sync::mpsc::{
    channel,
    Receiver,
    Sender
};

pub struct Broadcaster {
    clients: Vec<Sender<web::Bytes>>,
}

impl Broadcaster {
    pub fn default() -> Self {
        Broadcaster {
            clients: Vec::new(),
        }
    }
    pub fn add_client(&mut self) -> Client {
        let (tx, rx) = channel(1);
        self.clients.push(tx);
        return Client(rx);
    }
    pub fn make_message_block(buffer: &Vector<u8>) -> Vec<u8> {
        let bfu8 = buffer.as_ref();
        let mut msg = format!("--boundarydonotcross\r\nContent-Length:{}\r\nContent-Type:image/jpeg\r\n\r\n", bfu8.len()).into_bytes();
        msg.extend(bfu8);
        msg
    }
    fn send_image(&mut self, msg: &[u8]) {
        let mut ok_clients = Vec::new();
        let msg = web::Bytes::from([msg].concat());
        for client in self.clients.iter() {
            let result = client.clone().try_send(msg.clone());
            if let Ok(()) = result {
                ok_clients.push(client.clone());
            }
        }
        self.clients = ok_clients;
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