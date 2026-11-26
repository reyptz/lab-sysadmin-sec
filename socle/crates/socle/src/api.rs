//! API REST (`/api/v1`) et service de la console. Tous les points d'entrée
//! délèguent au même [`Engine`] que la CLI : aucune logique métier ici.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use socle_core::CoreError;
use socle_core::duration::HumanDuration;
use socle_core::incident::{AlertmanagerPayload, IncidentStatus, NewIncident};
use socle_core::operation::{ApprovalDecision, OperationRequest};
use socle_core::rbac::Principal;
use socle_core::release::NewRelease;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;

use crate::auth::Authenticator;
use crate::console;
use crate::engine::{Engine, IncidentPatch};
use crate::error::Error;
use crate::model::OperationFilter;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub auth: Arc<Authenticator>,
}

// ---------------------------------------------------------------------------
// Erreurs
// ---------------------------------------------------------------------------

pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        let (status, code) = match &e {
            Error::Core(CoreError::InvalidSpec(_)) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_spec"),
            Error::Core(CoreError::InvalidRequest(_)) => (StatusCode::BAD_REQUEST, "invalid_request"),
            Error::Core(CoreError::NotFound(_)) => (StatusCode::NOT_FOUND, "not_found"),
            Error::Core(CoreError::Forbidden(_)) => (StatusCode::FORBIDDEN, "forbidden"),
            Error::Core(CoreError::InvalidTransition(_)) | Error::Core(CoreError::Conflict(_)) | Error::Stale(_) => {
                (StatusCode::CONFLICT, "conflict")
            }
            Error::Unauthenticated(_) => (StatusCode::UNAUTHORIZED, "unauthenticated"),
            Error::Db(_) | Error::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            // Aucun détail interne n'est renvoyé au client.
            let reference = uuid::Uuid::new_v4().simple().to_string();
            tracing::error!(référence = %reference, "erreur interne : {e}");
            format!("erreur interne (référence {})", &reference[..12])
        } else {
            e.to_string()
        };
        Self { status, code, message }
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        Error::Core(e).into()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut res =
            (self.status, Json(json!({"error": {"code": self.code, "message": self.message}}))).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            res.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        res
    }
}

type ApiResult<T> = Result<T, ApiError>;

// ---------------------------------------------------------------------------
// Routeur
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/whoami", get(whoami))
        .route("/overview", get(overview))
        .route("/environments", get(environments))
        .route("/spec", get(spec))
        .route("/spec/reload", post(reload_spec))
        .route("/releases", get(list_releases).post(register_release))
        .route("/operations", get(list_operations).post(submit_operation))
        .route("/operations/preview", post(preview_operation))
        .route("/operations/{id}", get(get_operation))
        .route("/operations/{id}/logs", get(operation_logs))
        .route("/operations/{id}/approve", post(approve_operation))
        .route("/operations/{id}/reject", post(reject_operation))
        .route("/operations/{id}/cancel", post(cancel_operation))
        .route("/incidents", get(list_incidents).post(open_incident))
        .route("/incidents/{id}", get(get_incident).patch(update_incident))
        .route("/incidents/{id}/transition", post(transition_incident))
        .route("/incidents/{id}/notes", post(add_note))
        .route("/incidents/{id}/postmortem", get(postmortem))
        .route("/webhooks/alertmanager", post(alertmanager_webhook))
        .route("/backups", get(backups))
        .route("/maintenance", get(list_maintenance).post(start_maintenance))
        .route("/maintenance/{id}/end", post(end_maintenance))
        .route("/audit", get(audit_events))
        .route("/audit/verify", get(audit_verify))
        .route("/audit/export", get(audit_export))
        .route("/compliance", get(compliance))
        .route("/tokens", get(list_tokens).post(create_token))
        .route("/tokens/{id}", axum::routing::delete(revoke_token))
        .route("/locks", get(locks))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate));

    Router::new()
        .route("/", get(console::index))
        .route("/assets/app.js", get(console::script))
        .route("/assets/app.css", get(console::style))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .nest("/api/v1", api)
        .fallback(|| async { ApiError::from(CoreError::not_found("ressource")) })
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(60)))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; \
                 frame-ancestors 'none'; base-uri 'none'; form-action 'self'; object-src 'none'",
            ),
        ))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer")))
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .with_state(state)
}

async fn authenticate(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(str::to_string);
    match state.auth.authenticate(header.as_deref()).await {
        Ok(p) => {
            req.extensions_mut().insert(p);
            next.run(req).await
        }
        Err(e) => ApiError::from(e).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Santé et métriques
// ---------------------------------------------------------------------------

async fn readyz(State(s): State<AppState>) -> ApiResult<&'static str> {
    s.engine.store().ping().await?;
    Ok("ready")
}

fn esc(label: &str) -> String {
    label.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ")
}

async fn metrics(State(s): State<AppState>) -> ApiResult<Response> {
    let system = Principal::system("metrics");
    let mut out = String::new();
    let _ = writeln!(out, "# HELP socle_build_info Version de socle.\n# TYPE socle_build_info gauge");
    let _ = writeln!(out, "socle_build_info{{version=\"{}\"}} 1", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "# HELP socle_operations Opérations par statut.\n# TYPE socle_operations gauge");
    for (status, n) in s.engine.store().count_operations_by_status().await? {
        let _ = writeln!(out, "socle_operations{{status=\"{}\"}} {n}", esc(&status));
    }
    let _ =
        writeln!(out, "# HELP socle_incidents_open Incidents actifs par gravité.\n# TYPE socle_incidents_open gauge");
    let incidents = s.engine.store().list_incidents(true, 500).await?;
    for sev in ["sev1", "sev2", "sev3", "sev4"] {
        let n = incidents.iter().filter(|i| i.severity.as_str() == sev).count();
        let _ = writeln!(out, "socle_incidents_open{{severity=\"{sev}\"}} {n}");
    }
    let _ = writeln!(
        out,
        "# HELP socle_backup_rpo_compliant 1 si le RPO de la politique est tenu.\n# TYPE socle_backup_rpo_compliant gauge\n\
         # HELP socle_backup_last_success_age_seconds Âge de la dernière sauvegarde réussie.\n# TYPE socle_backup_last_success_age_seconds gauge\n\
         # HELP socle_backup_drill_within_rto 1 si le dernier exercice a tenu le RTO.\n# TYPE socle_backup_drill_within_rto gauge"
    );
    for b in s.engine.backup_statuses(&system).await? {
        let labels = format!("policy=\"{}\",environment=\"{}\"", esc(&b.policy), esc(&b.environment));
        let _ = writeln!(out, "socle_backup_rpo_compliant{{{labels}}} {}", u8::from(b.rpo_compliant));
        if let Some(age) = b.age_secs {
            let _ = writeln!(out, "socle_backup_last_success_age_seconds{{{labels}}} {age}");
        }
        if let Some(ok) = b.drill.as_ref().and_then(|d| d.within_rto) {
            let _ = writeln!(out, "socle_backup_drill_within_rto{{{labels}}} {}", u8::from(ok));
        }
    }
    Ok(([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], out).into_response())
}

// ---------------------------------------------------------------------------
// Vues
// ---------------------------------------------------------------------------

async fn whoami(State(s): State<AppState>, Extension(p): Extension<Principal>) -> Json<Value> {
    let mut v = serde_json::to_value(s.engine.whoami(&p)).unwrap_or(Value::Null);
    v["oidc"] = json!(s.auth.oidc_enabled());
    Json(v)
}

async fn overview(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(s.engine.overview(&p).await?).map_err(Error::from)?))
}

async fn environments(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::to_value(s.engine.environments(&p).await?).map_err(Error::from)?))
}

async fn spec(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    let spec = s.engine.spec();
    s.engine.authorize(&spec, &p, socle_core::rbac::Permission::Read, None, None)?;
    let mut v = serde_json::to_value(&*spec).map_err(Error::from)?;
    v["hash"] = json!(spec.hash());
    Ok(Json(v))
}

async fn reload_spec(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"hash": s.engine.reload_spec(&p).await?})))
}

// ---------------------------------------------------------------------------
// Releases et opérations
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ReleaseQuery {
    service: Option<String>,
    limit: Option<i64>,
}

async fn list_releases(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<ReleaseQuery>,
) -> ApiResult<Json<Value>> {
    let r = s.engine.list_releases(&p, q.service.as_deref(), q.limit.unwrap_or(50)).await?;
    Ok(Json(json!(r)))
}

async fn register_release(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(body): Json<NewRelease>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let r = s.engine.register_release(&p, body).await?;
    Ok((StatusCode::CREATED, Json(json!(r))))
}

async fn list_operations(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(f): Query<OperationFilter>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.list_operations(&p, &f).await?)))
}

async fn submit_operation(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(req): Json<OperationRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let op = s.engine.submit(&p, req).await?;
    Ok((StatusCode::CREATED, Json(json!(op))))
}

async fn preview_operation(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(req): Json<OperationRequest>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.preview(&p, &req).await?)))
}

async fn get_operation(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.get_operation(&p, &id).await?)))
}

#[derive(Deserialize)]
struct LogQuery {
    after: Option<i64>,
    limit: Option<i64>,
}

async fn operation_logs(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    Query(q): Query<LogQuery>,
) -> ApiResult<Json<Value>> {
    let logs = s.engine.operation_logs(&p, &id, q.after.unwrap_or(0), q.limit.unwrap_or(1_000)).await?;
    Ok(Json(json!(logs)))
}

#[derive(Deserialize, Default)]
struct Comment {
    #[serde(default)]
    comment: String,
}

async fn approve_operation(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    body: Option<Json<Comment>>,
) -> ApiResult<Json<Value>> {
    let c = body.map(|b| b.0).unwrap_or_default();
    Ok(Json(json!(s.engine.decide(&p, &id, ApprovalDecision::Approved, &c.comment).await?)))
}

async fn reject_operation(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    Json(c): Json<Comment>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.decide(&p, &id, ApprovalDecision::Rejected, &c.comment).await?)))
}

#[derive(Deserialize, Default)]
struct Reason {
    #[serde(default)]
    reason: String,
}

async fn cancel_operation(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    body: Option<Json<Reason>>,
) -> ApiResult<Json<Value>> {
    let r = body.map(|b| b.0).unwrap_or_default();
    Ok(Json(json!(s.engine.cancel(&p, &id, &r.reason).await?)))
}

// ---------------------------------------------------------------------------
// Incidents
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct IncidentQuery {
    active: Option<bool>,
    limit: Option<i64>,
}

async fn list_incidents(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<IncidentQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.list_incidents(&p, q.active.unwrap_or(false), q.limit.unwrap_or(100)).await?)))
}

async fn open_incident(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(body): Json<NewIncident>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    Ok((StatusCode::CREATED, Json(json!(s.engine.open_incident(&p, body).await?))))
}

async fn get_incident(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.get_incident(&p, &id).await?)))
}

async fn update_incident(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    Json(patch): Json<IncidentPatch>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.update_incident(&p, &id, patch).await?)))
}

#[derive(Deserialize)]
struct Transition {
    status: String,
    #[serde(default)]
    note: Option<String>,
}

async fn transition_incident(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    Json(t): Json<Transition>,
) -> ApiResult<Json<Value>> {
    let status = IncidentStatus::parse(&t.status)
        .ok_or_else(|| CoreError::invalid(format!("statut inconnu « {} »", t.status)))?;
    Ok(Json(json!(s.engine.transition_incident(&p, &id, status, t.note.as_deref()).await?)))
}

#[derive(Deserialize)]
struct Note {
    note: String,
}

async fn add_note(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    Json(n): Json<Note>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.add_incident_note(&p, &id, &n.note).await?)))
}

async fn postmortem(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let incident = s.engine.get_incident(&p, &id).await?;
    Ok(([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], incident.render_postmortem()).into_response())
}

async fn alertmanager_webhook(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(payload): Json<AlertmanagerPayload>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"incidents": s.engine.ingest_alerts(&p, &payload).await?})))
}

// ---------------------------------------------------------------------------
// Sauvegardes, maintenance
// ---------------------------------------------------------------------------

async fn backups(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.backup_statuses(&p).await?)))
}

#[derive(Deserialize)]
struct MaintenanceQuery {
    active: Option<bool>,
}

async fn list_maintenance(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<MaintenanceQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.list_maintenance(&p, q.active.unwrap_or(false)).await?)))
}

#[derive(Deserialize)]
struct StartMaintenance {
    environment: String,
    duration: HumanDuration,
    reason: String,
}

async fn start_maintenance(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(b): Json<StartMaintenance>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let m = s.engine.start_maintenance(&p, &b.environment, b.duration, &b.reason).await?;
    Ok((StatusCode::CREATED, Json(json!(m))))
}

async fn end_maintenance(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.end_maintenance(&p, &id).await?)))
}

// ---------------------------------------------------------------------------
// Audit et conformité
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AuditQuery {
    after: Option<i64>,
    limit: Option<i64>,
}

async fn audit_events(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.audit_events(&p, q.after, q.limit.unwrap_or(100)).await?)))
}

async fn audit_verify(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.verify_audit(&p).await?)))
}

async fn audit_export(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Response> {
    let mut body = String::new();
    let mut after = 0;
    loop {
        let batch = s.engine.audit_events(&p, Some(after), 5_000).await?;
        let Some(last) = batch.last() else { break };
        after = last.seq;
        for e in &batch {
            body.push_str(&serde_json::to_string(e).map_err(Error::from)?);
            body.push('\n');
        }
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"socle-audit.jsonl\""),
        ],
        body,
    )
        .into_response())
}

#[derive(Deserialize)]
struct ComplianceQuery {
    days: Option<i64>,
    format: Option<String>,
}

async fn compliance(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<ComplianceQuery>,
) -> ApiResult<Response> {
    let report = s.engine.compliance_report(&p, q.days.unwrap_or(90)).await?;
    if q.format.as_deref() == Some("md") {
        return Ok(([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], report.to_markdown()).into_response());
    }
    Ok(Json(json!(report)).into_response())
}

// ---------------------------------------------------------------------------
// Jetons et verrous
// ---------------------------------------------------------------------------

async fn list_tokens(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.engine.list_tokens(&p).await?)))
}

#[derive(Deserialize)]
struct NewToken {
    name: String,
    subject: String,
    teams: Vec<String>,
    #[serde(default)]
    ttl: Option<HumanDuration>,
}

async fn create_token(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(b): Json<NewToken>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (token, secret) = s.engine.create_token(Some(&p), &b.name, &b.subject, b.teams, b.ttl).await?;
    Ok((StatusCode::CREATED, Json(json!({"token": token, "secret": secret}))))
}

async fn revoke_token(
    State(s): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    s.engine.revoke_token(&p, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn locks(State(s): State<AppState>, Extension(p): Extension<Principal>) -> ApiResult<Json<Value>> {
    let spec = s.engine.spec();
    s.engine.authorize(&spec, &p, socle_core::rbac::Permission::Read, None, None)?;
    Ok(Json(json!(s.engine.store().list_locks().await?)))
}
