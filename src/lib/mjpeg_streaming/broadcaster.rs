// Inspired by https://github.com/dskkato/mjpeg-rs#live-streaming-server-with-rustactix-web

use std::thread;
use std::sync::Mutex;
use std::pin::Pin;
use std::task::{
    Context,
    Poll
};
use std::sync::mpsc::{
    Receiver as STDReceiver
};

use actix_web::{
    web
};
use actix_web::Error;

use futures::Stream;
use tokio::sync::mpsc::{
    channel,
    Receiver,
    Sender
};

use image;

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
    pub fn make_message_block(frame: &[u8], width: u32, height: u32) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut buffer);
        encoder.encode(&frame, width, height, image::ColorType::Rgb8).unwrap();
        let mut msg = format!("--boundarydonotcross\r\nContent-Length:{}\r\nContent-Type:image/jpeg\r\n\r\n", buffer.len()).into_bytes();
        msg.extend(buffer);
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
    pub fn spawn_reciever(_self: web::Data<Mutex<Self>>, rx_frames_data: STDReceiver<std::vec::Vec<u8>>, width: u32, height: u32) {
        thread::spawn(move || {
            for received in rx_frames_data {
                let msg = Broadcaster::make_message_block(&received, width, height);
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