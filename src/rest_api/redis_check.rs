use std::time::{Duration, Instant};

use actix_web::{Error, HttpResponse, web};
use redis::Client;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

use crate::lib::logging;
use crate::rest_api::APIStorage;

/// A check must answer quickly: it is there to tell a typo from a firewall,
/// not to wait out a dead host
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Where to try. Anything left out is taken from the current configuration, so
/// an empty body checks what the app is set up to use
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct RedisCheckRequest {
    pub host: Option<String>,
    pub port: Option<i32>,
    pub db_index: Option<i32>,
    /// Leave it out to use the password already in the configuration
    pub password: Option<String>,
}

/// Whether Redis answered
#[derive(Debug, Serialize, ToSchema)]
pub struct RedisCheckResponse {
    pub ok: bool,
    /// How long the connection and the PING took
    pub took_ms: u64,
    /// Where it tried, without the password
    #[schema(example = "localhost:6379/0")]
    pub target: String,
    /// Why it did not work, `null` when it did
    pub error: Option<String>,
}

#[utoipa::path(
    post,
    tag = "Configuration",
    path = "/api/redis/check",
    request_body = RedisCheckRequest,
    responses(
        (status = 200, description = "The attempt was made; `ok` says how it went", body = RedisCheckResponse)
    )
)]
/// Tries to connect to Redis and PING it, so that a host, a port or a password
/// can be checked before it is saved. Answers 200 either way: the request
/// itself succeeded, `ok` says whether Redis did
pub async fn check_redis(
    data: web::Data<APIStorage>,
    request: Option<web::Json<RedisCheckRequest>>,
) -> Result<HttpResponse, Error> {
    let request = request.map(web::Json::into_inner).unwrap_or_default();
    let (host, port, db_index, password) = {
        let settings = data
            .app_settings
            .read()
            .expect("Settings are poisoned [RwLock]");
        let redis = &settings.redis_publisher;
        (
            request.host.unwrap_or_else(|| redis.host.clone()),
            request.port.unwrap_or(redis.port),
            request.db_index.unwrap_or(redis.db_index),
            request.password.unwrap_or_else(|| redis.password.clone()),
        )
    };
    let target = format!("{host}:{port}/{db_index}");

    let url = if password.is_empty() {
        format!("redis://{host}:{port}/{db_index}")
    } else {
        format!("redis://:{password}@{host}:{port}/{db_index}")
    };
    // Connecting blocks, and the worker thread has other requests to serve
    let outcome = web::block(move || {
        let started = Instant::now();
        let result = Client::open(url)
            .and_then(|client| client.get_connection_with_timeout(CONNECT_TIMEOUT))
            .and_then(|mut connection| redis::cmd("PING").query::<String>(&mut connection));
        (result, started.elapsed())
    })
    .await;

    let (result, took) = match outcome {
        Ok(outcome) => outcome,
        Err(err) => {
            return Ok(HttpResponse::Ok().json(RedisCheckResponse {
                ok: false,
                took_ms: 0,
                target,
                error: Some(format!("Can't run the check: {err}")),
            }));
        }
    };
    let error = result.err().map(|err| err.to_string());
    info!(
        scope = logging::SCOPE_REDIS,
        target = %target,
        ok = error.is_none(),
        took_ms = took.as_millis() as u64,
        "Redis check"
    );
    Ok(HttpResponse::Ok().json(RedisCheckResponse {
        ok: error.is_none(),
        took_ms: took.as_millis() as u64,
        target,
        error,
    }))
}
