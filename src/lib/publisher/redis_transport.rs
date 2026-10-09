use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

use redis::{Client, Commands, Connection, ConnectionAddr, ConnectionInfo, RedisConnectionInfo};
use tracing::{info, warn};

use crate::lib::logging;
use crate::settings::RedisConnectionSettings;

const QUEUE_CAPACITY: usize = 64;
const IO_TIMEOUT: Duration = Duration::from_secs(3);

/// How to reach the configured Redis or Valkey server.
///
/// Built as a struct rather than formatted into a `redis://user:pass@host/db`
/// URL, because the crate percent-decodes whatever it finds in a URL: a
/// password holding `@`, `/` or `%` is then read as part of the host or the
/// path. Measured against Valkey 8.1 - `p@ss/w0rd` makes the parser take `ss`
/// for the host and give up with "Invalid database number"
pub fn connection_info(
    host: &str,
    port: i32,
    db_index: i32,
    username: Option<&str>,
    password: &str,
) -> ConnectionInfo {
    let some_if_set = |value: &str| match value.is_empty() {
        true => None,
        false => Some(value.to_string()),
    };
    ConnectionInfo {
        addr: ConnectionAddr::Tcp(host.to_string(), port.max(0) as u16),
        redis: RedisConnectionInfo {
            db: db_index as i64,
            username: username.and_then(some_if_set),
            password: some_if_set(password),
            ..Default::default()
        },
    }
}

struct PendingMessage {
    channel: String,
    serialize: Box<dyn FnOnce() -> Result<String, serde_json::Error> + Send>,
}

#[derive(Clone)]
pub struct RedisPublisher {
    sender: SyncSender<PendingMessage>,
    channel: String,
}

impl RedisPublisher {
    pub fn new(settings: &RedisConnectionSettings) -> Option<Self> {
        let client = match Client::open(connection_info(
            &settings.host,
            settings.port,
            settings.db_index,
            settings.username.as_deref(),
            &settings.password,
        )) {
            Ok(client) => client,
            Err(error) => {
                warn!(scope = logging::SCOPE_REDIS, %error, "Can't configure Redis publisher");
                return None;
            }
        };
        let (sender, receiver) = mpsc::sync_channel::<PendingMessage>(QUEUE_CAPACITY);
        let worker = thread::Builder::new().name("redis-publisher".into()).spawn(move || {
            let mut connection: Option<Connection> = None;
            for message in receiver {
                let payload = match (message.serialize)() {
                    Ok(payload) => payload,
                    Err(error) => {
                        warn!(scope = logging::SCOPE_REDIS, channel = %message.channel, %error, "Can't serialize Redis message");
                        continue;
                    }
                };
                let result = (|| -> redis::RedisResult<usize> {
                    if connection.is_none() {
                        let opened = client.get_connection_with_timeout(IO_TIMEOUT)?;
                        opened.set_read_timeout(Some(IO_TIMEOUT))?;
                        opened.set_write_timeout(Some(IO_TIMEOUT))?;
                        connection = Some(opened);
                    }
                    connection.as_mut().expect("connection established").publish(&message.channel, payload)
                })();
                match result {
                    Ok(subscribers) => {
                        info!(scope = logging::SCOPE_REDIS, channel = %message.channel, subscribers, "Published to Redis");
                    }
                    Err(error) => {
                        // A failed reply can follow a successful publish. Reconnect for the next message without replaying this one.
                        connection = None;
                        warn!(scope = logging::SCOPE_REDIS, channel = %message.channel, %error, "Redis publication failed; message discarded");
                    }
                }
            }
        });
        if let Err(error) = worker {
            warn!(scope = logging::SCOPE_REDIS, %error, "Can't start Redis publisher");
            return None;
        }
        Some(Self {
            sender,
            channel: String::new(),
        })
    }

    pub fn with_channel(&self, channel: &str) -> Self {
        Self {
            sender: self.sender.clone(),
            channel: channel.to_owned(),
        }
    }

    pub fn publish(
        &self,
        serialize: impl FnOnce() -> Result<String, serde_json::Error> + Send + 'static,
    ) {
        let message = PendingMessage {
            channel: self.channel.clone(),
            serialize: Box::new(serialize),
        };
        match self.sender.try_send(message) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                warn!(scope = logging::SCOPE_REDIS, channel = %self.channel, "Redis queue full; message discarded");
            }
            Err(TrySendError::Disconnected(_)) => {
                warn!(scope = logging::SCOPE_REDIS, channel = %self.channel, "Redis publisher stopped; message discarded");
            }
        }
    }
}
