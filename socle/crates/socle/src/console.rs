//! Console web : application monopage sans dépendance externe (aucun CDN,
//! compatible environnements isolés), embarquée dans le binaire et servie
//! sous une politique CSP stricte. Elle n'utilise que l'API publique.

use axum::http::header;
use axum::response::IntoResponse;

const INDEX: &str = include_str!("../assets/index.html");
const SCRIPT: &str = include_str!("../assets/app.js");
const STYLE: &str = include_str!("../assets/app.css");

pub async fn index() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], INDEX)
}

pub async fn script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], SCRIPT)
}

pub async fn style() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], STYLE)
}
