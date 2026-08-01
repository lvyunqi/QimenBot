use axum::body::Body;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, Response, StatusCode};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/admin/dist"]
struct AdminAssets;

pub async fn index() -> Response<Body> {
    asset_response("index.html")
}

pub async fn spa(axum::extract::Path(path): axum::extract::Path<String>) -> Response<Body> {
    if path.starts_with("api/") {
        return not_found();
    }
    if AdminAssets::get(&path).is_some() {
        asset_response(&path)
    } else {
        asset_response("index.html")
    }
}

fn asset_response(path: &str) -> Response<Body> {
    let Some(asset) = AdminAssets::get(path) else {
        return not_found();
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache_control = if path == "index.html" {
        "no-cache, no-store, must-revalidate"
    } else {
        "public, max-age=31536000, immutable"
    };
    let mut response = Response::new(Body::from(asset.data));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(cache_control));
    add_security_headers(&mut response);
    response
}

fn not_found() -> Response<Body> {
    let mut response = Response::new(Body::from("not found"));
    *response.status_mut() = StatusCode::NOT_FOUND;
    add_security_headers(&mut response);
    response
}

fn add_security_headers(response: &mut Response<Body>) {
    response
        .headers_mut()
        .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
    );
}
