use crate::http::user_token::TokenUser;
use crate::utils::http_error_handler::ErrorResponse;
use axum::extract::Request;
use axum::response::IntoResponse;
use axum::Extension;
use axum::{
    http::{header, StatusCode},
    response::Response,
};
use derive_builder::Builder;
use futures_util::future::BoxFuture;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tracing::error;

const AUTH_METHOD_KEY_JWT: &str = "Bearer";

#[derive(Clone)]
pub struct JwtAuthConfig {
    issuer: String,
    secret: String,
}

#[derive(Clone)]
pub struct MLayer {
    config: JwtAuthConfig,
}

pub fn new(config: JwtAuthConfig) -> MLayer {
    MLayer { config }
}

// TODO too many copy. may need refractor
impl<S> Layer<S> for MLayer {
    type Service = Middleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Middleware {
            inner,
            config: self.config.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Middleware<S> {
    inner: S,
    config: JwtAuthConfig,
}

#[derive(Debug, Serialize, Deserialize, Builder)]
pub(crate) struct JwtClaims {
    exp: usize,
    iss: String,
    iat: usize,
    cla: TokenUser,
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

    fn call(&mut self, mut request: Request) -> Self::Future {
        let mut proceed = true;
        if let Some((auth_header, token)) = self.extract_authentication_fields(&request) {
            match auth_header {
                AUTH_METHOD_KEY_JWT => match self.parse_token_user(token) {
                    Ok(token_user) => {
                        let _ = request.extensions_mut().insert(Extension(token_user));
                    }
                    Err(e) => {
                        error!(
                            token = token,
                            err = e.to_string(),
                            "parse authentication header error"
                        );
                        proceed = false;
                    }
                },
                _ => {}
            }
            // instead of x-token-user header
            request.headers_mut().remove(header::AUTHORIZATION);
        }
        if proceed {
            let future = self.inner.call(request);
            return Box::pin(async move {
                let response: Response = future.await?;
                Ok(response)
            });
        }
        Box::pin(async move {
            Ok(ErrorResponse::new_with_status_code(StatusCode::UNAUTHORIZED).into_response())
        })
    }
}

impl<S> Middleware<S> {
    fn extract_authentication_fields<'a>(&self, req: &'a Request) -> Option<(&'a str, &'a str)> {
        if let Some(val) = req.headers().get(header::AUTHORIZATION) {
            if let Ok(val) = val.to_str() {
                let splits: Vec<&str> = val.split(" ").collect();
                if splits.len() == 2 {
                    return Some((splits[0], splits[1]));
                }
            }
        }
        return None;
    }

    fn parse_token_user(&self, token: &str) -> Result<TokenUser, anyhow::Error> {
        let mut validator = Validation::new(jsonwebtoken::Algorithm::HS512);
        validator.set_issuer(&vec![&self.config.issuer]);
        Ok(decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(self.config.secret.as_bytes()),
            &validator,
        )?
        .claims
        .cla)
    }
}
