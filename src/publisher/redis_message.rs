use std::error::Error;

pub trait RedisMessage {
    fn prepare_string(&self) -> Result<String, Box<dyn Error>>;
}