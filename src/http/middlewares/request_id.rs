use crate::http::header;
use axum::response::Response;
use axum::{extract::Request, http::HeaderValue};
use futures_util::future::BoxFuture;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Clone)]
pub struct MLayer {}

pub fn new() -> MLayer {
    MLayer {}
}

impl<S> Layer<S> for MLayer {
    type Service = Middleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Middleware { inner }
    }
}

#[derive(Clone)]
pub struct Middleware<S> {
    inner: S,
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
        if let Ok(header_val) = HeaderValue::from_str(&uuid::Uuid::new_v4().to_string())
            .inspect_err(|e| {
                eprintln!("generate request id error {}", e);
            })
        {
            request
                .headers_mut()
                .append(header::X_REQUEST_ID, header_val);
        }
        let future = self.inner.call(request);
        return Box::pin(async move {
            let response: Response = future.await?;
            Ok(response)
        });
    }
}

impl<S> Middleware<S> {}
