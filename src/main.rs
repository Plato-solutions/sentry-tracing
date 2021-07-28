use actix_web::{web};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::prelude::*;
use span_mapper::custom_span_mapper;
use handlers::service_endpoint;

mod span_mapper;
mod handlers;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(service_endpoint);
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let dsn = std::env::var("SENTRY_DSN").expect("No dsn in env");
    let _guard = sentry::init((dsn, sentry::ClientOptions {
        release: sentry::release_name!(),
        traces_sample_rate: 1.0,
        ..Default::default()
    }));
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(sentry_tracing::layer()
            .span_mapper(custom_span_mapper))
        .init();
    
    actix_web::HttpServer::new(move || {
        let app = actix_web::App::new();
        app.configure(init)

    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}