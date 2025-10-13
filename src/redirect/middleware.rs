use axum::{body::Body, http::Request, middleware::Next, response::Response};
use std::time::Instant;

#[derive(Copy, Clone)]
pub struct RequestStart(pub Instant);

pub async fn record_request_start(mut request: Request<Body>, next: Next) -> Response {
    request
        .extensions_mut()
        .insert(RequestStart(Instant::now()));
    next.run(request).await
}
