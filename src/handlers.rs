use actix_web::{web, HttpResponse, get, Error};
use tokio::time::sleep;

#[tracing::instrument]
async fn first() {
    tracing::error!("Generates an event");
}

#[tracing::instrument]
async fn second() {
    sleep(std::time::Duration::from_millis(100)).await;
}

#[get("/api/service_endpoint")]
async fn service_endpoint(req: web::HttpRequest) -> Result<HttpResponse, Error> {
    use tracing::{Level, span};
    let (trace_id, span_id) = if let Some(h) = req.headers().get("sentry-trace") {
        let mut parts = h.to_str().unwrap().split("-");
        (parts.next(), parts.next())
    } else {
        (None, None)
    };
    {
        let my_span = match (trace_id, span_id) {
            (Some(trace_id), Some(span_id)) => {
                span!(Level::INFO, "server_side_transaction", trace_id = trace_id, span_id = span_id)
            }
            _ => {
                span!(Level::INFO, "server_side_transaction")
            }
        };
        let _guard = my_span.enter();
        first().await;
        second().await;
    }
    Ok(HttpResponse::Ok().body("OK."))
}
