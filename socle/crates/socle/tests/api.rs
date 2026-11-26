//! Tests de l'API HTTP : authentification, RBAC, en-têtes de sécurité,
//! cycle d'une opération, métriques et conformité.

#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use common::{good_release, harness};
use serde_json::{Value, json};
use socle::api::{AppState, router};
use socle::auth::Authenticator;
use tower::ServiceExt;

struct Api {
    app: axum::Router,
    admin: String,
    erp: String,
    viewer: String,
    _h: common::Harness,
}

async fn api() -> Api {
    let h = harness().await;
    let e = &h.engine;
    let (_, admin) = e.create_token(None, "admin", "root@example.org", vec!["platform".into()], None).await.unwrap();
    let (_, erp) = e.create_token(None, "ci", "github:acme/erp", vec!["erp".into()], None).await.unwrap();
    let (_, viewer) = e.create_token(None, "lecture", "auditor@example.org", vec!["audit".into()], None).await.unwrap();
    let auth = Arc::new(Authenticator::new(Arc::clone(&h.engine), None));
    Api { app: router(AppState { engine: Arc::clone(&h.engine), auth }), admin, erp, viewer, _h: h }
}

async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())).unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, headers, bytes)
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

#[tokio::test]
async fn public_endpoints_and_security_headers() {
    let a = api().await;
    let (s, _, body) = call(&a.app, "GET", "/healthz", None, None).await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, b"ok".as_slice()));
    let (s, headers, body) = call(&a.app, "GET", "/", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains("/assets/app.js"));
    let csp = headers.get(header::CONTENT_SECURITY_POLICY).unwrap().to_str().unwrap();
    assert!(csp.contains("script-src 'self'") && csp.contains("frame-ancestors 'none'"));
    assert_eq!(headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    let (s, _, body) = call(&a.app, "GET", "/metrics", None, None).await;
    assert_eq!(s, StatusCode::OK);
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("socle_build_info") && text.contains("socle_backup_rpo_compliant"));
}

#[tokio::test]
async fn authentication_is_required() {
    let a = api().await;
    let (s, headers, body) = call(&a.app, "GET", "/api/v1/whoami", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(headers.get(header::WWW_AUTHENTICATE).unwrap(), "Bearer");
    assert_eq!(json_of(&body)["error"]["code"], "unauthenticated");
    let (s, _, _) = call(&a.app, "GET", "/api/v1/whoami", Some("socle_pat_faux"), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _, body) = call(&a.app, "GET", "/api/v1/whoami", Some(&a.admin), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(json_of(&body)["is_admin"], true);
}

#[tokio::test]
async fn operation_lifecycle_over_http() {
    let a = api().await;
    let release = serde_json::to_value(good_release("1.0.0", 'a')).unwrap();
    let (s, _, body) = call(&a.app, "POST", "/api/v1/releases", Some(&a.erp), Some(release)).await;
    assert_eq!(s, StatusCode::CREATED, "{}", String::from_utf8_lossy(&body));

    let req = json!({"environment": "dev", "action": {"kind": "deploy", "service": "erp-app", "release": "1.0.0"}, "reason": "CI"});
    let (s, _, body) = call(&a.app, "POST", "/api/v1/operations/preview", Some(&a.erp), Some(req.clone())).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(json_of(&body)["report"]["decision"], "proceed");

    let (s, _, body) = call(&a.app, "POST", "/api/v1/operations", Some(&a.erp), Some(req.clone())).await;
    assert_eq!(s, StatusCode::CREATED);
    let id = json_of(&body)["id"].as_str().unwrap().to_string();
    let (s, _, body) = call(&a.app, "GET", &format!("/api/v1/operations/{id}"), Some(&a.viewer), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(json_of(&body)["status"], "queued");

    // RBAC : un lecteur ne peut pas déployer ; un déployeur ne lit pas l'audit.
    let (s, _, body) = call(&a.app, "POST", "/api/v1/operations", Some(&a.viewer), Some(req)).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(json_of(&body)["error"]["code"], "forbidden");
    let (s, _, _) = call(&a.app, "GET", "/api/v1/audit", Some(&a.erp), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // Requête invalide : champ inconnu refusé.
    let bad =
        json!({"environment": "dev", "action": {"kind": "deploy", "service": "erp-app", "release": "1.0.0", "x": 1}});
    let (s, _, _) = call(&a.app, "POST", "/api/v1/operations", Some(&a.erp), Some(bad)).await;
    assert!(s.is_client_error());

    let (s, _, body) = call(&a.app, "GET", "/api/v1/audit/verify", Some(&a.admin), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(json_of(&body)["valid"], true);

    let (s, headers, body) = call(&a.app, "GET", "/api/v1/compliance?days=30&format=md", Some(&a.admin), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(headers.get(header::CONTENT_TYPE).unwrap().to_str().unwrap().starts_with("text/markdown"));
    assert!(String::from_utf8_lossy(&body).contains("ISO 27001 A.8.32"));
}

#[tokio::test]
async fn incidents_over_http() {
    let a = api().await;
    let (s, _, body) = call(
        &a.app,
        "POST",
        "/api/v1/incidents",
        Some(&a.admin),
        Some(json!({"title": "Latence élevée", "severity": "sev2", "environment": "prod"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let id = json_of(&body)["id"].as_str().unwrap().to_string();
    let (s, _, _) = call(
        &a.app,
        "POST",
        &format!("/api/v1/incidents/{id}/transition"),
        Some(&a.admin),
        Some(json!({"status": "resolved"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // Clôture d'un Sev2 sans cause racine : refusée.
    let (s, _, _) = call(
        &a.app,
        "POST",
        &format!("/api/v1/incidents/{id}/transition"),
        Some(&a.admin),
        Some(json!({"status": "closed"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _, body) = call(&a.app, "GET", &format!("/api/v1/incidents/{id}/postmortem"), Some(&a.viewer), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).starts_with("# Postmortem"));
}
