use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::Response,
};
use mime_guess::from_path;
use rust_embed::RustEmbed;
use std::path::PathBuf;

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
pub struct Assets;

/// Serve static files from embedded assets or filesystem
pub async fn serve_static(uri: Uri, static_dir: Option<String>) -> Response {
    let path = uri.path().trim_start_matches('/');
    
    // Try to serve from filesystem if static_dir is provided
    if let Some(ref dir) = static_dir {
        let file_path = PathBuf::from(dir).join(path);
        if let Ok(content) = tokio::fs::read(&file_path).await {
            let mime_type = from_path(&file_path).first_or_octet_stream();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_type.as_ref())
                .body(Body::from(content))
                .unwrap();
        }
    }

    // Fall back to embedded assets
    serve_embedded(path).await
}

/// Serve from embedded assets
async fn serve_embedded(path: &str) -> Response {
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let mime = from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data))
                .unwrap()
        }
        None => {
            // For SPA routing, serve index.html for non-file paths
            if !path.contains('.') {
                if let Some(index) = Assets::get("index.html") {
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "text/html")
                        .body(Body::from(index.data))
                        .unwrap();
                }
            }
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))
                .unwrap()
        }
    }
}
