use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub async fn server_new() {
    // init logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // axum logs rejections from built-in extractors with the `axum::rejection`
            // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
            "info".into()
            // "user-app=debug,tower_http=debug,axum::rejection=trace".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
