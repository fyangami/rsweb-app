use super::super::super::config::{RateLimitConfig, RateLimiter};
use axum::response::{IntoResponse, Response};
use axum::{extract::Request, http::StatusCode};
use common::error_handler::ErrorResponse;
use common::hash::SignedContent;
use common::http_headers;
use common::redis_utils::MustLoadScript;
use fred::clients::{RedisClient, RedisPool};
use fred::types::RedisValue;
use futures_util::future::BoxFuture;
use std::{
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};
use tracing::{debug, error, info};

const RATE_LIMITER_KEY_BASE_PREFIX: &str = "gateway:rate_limiter:";
const ACQUIRE_PERMITTED: i32 = 1;
const RATE_LIMITER_SCRIPT: &str = r"
            local current_time = redis.call('TIME')
            local trim_time = tonumber(current_time[1]) - ARGV[2]
            redis.call('ZREMRANGEBYSCORE', ARGV[1], 0, trim_time)
            local request_count = redis.call('ZCARD', ARGV[1])

            if request_count < tonumber(ARGV[3]) then
                redis.call('ZADD', ARGV[1], current_time[1], current_time[1] .. current_time[2])
                redis.call('EXPIRE', ARGV[1], ARGV[2])
                return 1
            end
            return 0
        ";

#[derive(Clone)]
pub struct MLayer {
    redis: RedisPool,
    config: RateLimitConfig,
}

pub fn new(config: RateLimitConfig, redis: RedisPool) -> MLayer {
    MLayer { redis, config }
}

// TODO too many clone. may need optimizing for lower memory used.
impl<S> Layer<S> for MLayer {
    type Service = Middleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Middleware {
            inner,
            redis: self.redis.clone(),
            config: Arc::new(self.config.clone()),
            rate_limiter_script: Arc::new(MustLoadScript::new(RATE_LIMITER_SCRIPT)),
        }
    }
}

#[derive(Clone)]
pub struct Middleware<S> {
    inner: S,
    config: Arc<RateLimitConfig>,
    redis: RedisPool,
    rate_limiter_script: Arc<MustLoadScript>,
}

fn build_limiter_key(limiter: &RateLimiter, ip: &str) -> String {
    let mut key = String::from(RATE_LIMITER_KEY_BASE_PREFIX);
    // let ip = ip.unwrap_or("UNKNOWN-IP".to_owned());
    if limiter.scope_ip {
        key.push_str(&format!("ip:{}:", ip));
    }
    key.push_str(&limiter.path);
    key
}

async fn acquire_permit(
    redis: &RedisPool,
    limiter: &RateLimiter,
    script: &MustLoadScript,
    ip: &str,
) -> bool {
    let key = build_limiter_key(limiter, ip);
    // script must be loaded
    match script.get(redis).await {
        Ok(script) => {
            match script
                .evalsha::<i32, _, _, _>(
                    redis,
                    None,
                    vec![
                        RedisValue::from(&key),
                        RedisValue::from(limiter.interval_sec),
                        RedisValue::from(limiter.permits),
                    ],
                )
                .await
            {
                Ok(acquired) => {
                    info!(key = key, ip = ip, acquired = acquired, "acquiring permit");
                    acquired == ACQUIRE_PERMITTED
                }
                Err(e) => {
                    error!(key = key, limiter = ?limiter, err = ?e, "acquire permit error");
                    false
                }
            }
        }
        Err(e) => {
            error!(e = ?e, "load script error");
            false
        }
    }
}

impl<S> Service<Request> for Middleware<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let path = request.uri().path().to_owned();
        let ip = super::extract_ip_from_request(&request).to_owned();
        let forward_key = request
            .headers()
            .get(http_headers::X_RATE_LIMIT_FORWARD)
            .and_then(|x| Some(x.to_str().unwrap_or("").to_owned()));
        // nothing to do in `call` is invoked according to tower document.
        // review required in the after soon.
        let future = self.inner.call(request);
        let config = self.config.clone();
        let redis = self.redis.clone();
        let script = self.rate_limiter_script.clone();
        // let ip = request
        return Box::pin(async move {
            let mut proceed = true;
            // given a forward key will bypass rate limiter
            if let Some(forward_key) = forward_key {
                proceed = test_forward_key(
                    &forward_key,
                    &path,
                    &config.forward_key_secret,
                    &redis.next(),
                )
                .await;
                debug!(proceed = proceed, "got forward key");
            } else {
                // TODO refactor to use paralleling
                for limiter in config.limiters.iter() {
                    debug!(limiter = ?limiter, "handing limiter");
                    if limiter.strict && !path.eq(&limiter.path) {
                        continue;
                    }
                    if !path.starts_with(&limiter.path) {
                        continue;
                    }
                    if !acquire_permit(&redis, limiter, &script, &ip).await {
                        proceed = false;
                        break;
                    }
                }
            }
            if proceed {
                let response: Response = future.await?;
                return Ok(response);
            }
            Ok(ErrorResponse::new_with_status_code(StatusCode::TOO_MANY_REQUESTS).into_response())
        });
    }
}

impl<S> Middleware<S> {}

async fn test_forward_key(forward_key: &str, path: &str, secret: &str, rdb: &RedisClient) -> bool {
    SignedContent::parse_once_with_redis(forward_key, secret, rdb)
        .await
        .map(|signed_content: SignedContent<String>| {
            debug!(
                signed_content = signed_content.content,
                path = path,
                "inspect path permit"
            );
            signed_content.content.eq(path)
        })
        .inspect_err(|e| info!(e = ?e, "parse signed key error"))
        .unwrap_or(false)
}
