//! Notifications (Slack, Microsoft Teams, PagerDuty, webhook générique),
//! routées par type d'événement et par environnement. Un échec d'envoi est
//! journalisé mais n'interrompt jamais une opération.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use socle_core::incident::Severity;
use socle_core::spec::{Channel, Spec};

use crate::secrets::SecretResolver;

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub kind: String,
    pub environment: Option<String>,
    pub severity: Option<Severity>,
    pub title: String,
    pub text: String,
    /// Chemin relatif dans la console (ex. `#/operations/<id>`).
    pub link: Option<String>,
    pub data: Value,
    /// Clé de déduplication (PagerDuty) ; `resolve` clôt l'alerte associée.
    pub dedup_key: Option<String>,
    pub resolve: bool,
}

impl Event {
    pub fn new(kind: &str, title: impl Into<String>) -> Self {
        Self {
            kind: kind.to_string(),
            environment: None,
            severity: None,
            title: title.into(),
            text: String::new(),
            link: None,
            data: Value::Null,
            dedup_key: None,
            resolve: false,
        }
    }
}

pub struct Notifier {
    http: reqwest::Client,
    secrets: SecretResolver,
    public_url: Option<String>,
}

impl Notifier {
    pub fn new(public_url: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("socle/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self { http, secrets: SecretResolver, public_url: public_url.map(|u| u.trim_end_matches('/').to_string()) }
    }

    /// Envoi asynchrone (n'attend pas les canaux).
    pub fn dispatch(self: &Arc<Self>, spec: Arc<Spec>, event: Event) {
        let routes: Vec<String> = spec
            .notifications
            .routes
            .iter()
            .filter(|r| r.matches(&event.kind, event.environment.as_deref()))
            .flat_map(|r| r.channels.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if routes.is_empty() {
            return;
        }
        let this = Arc::clone(self);
        tokio::spawn(async move {
            for name in routes {
                if let Some(channel) = spec.notifications.channels.get(&name)
                    && let Err(e) = this.send(channel, &event).await
                {
                    tracing::warn!(canal = %name, événement = %event.kind, "notification non envoyée : {e}");
                }
            }
        });
    }

    fn link(&self, event: &Event) -> Option<String> {
        match (&self.public_url, &event.link) {
            (Some(base), Some(path)) => Some(format!("{base}/{}", path.trim_start_matches('/'))),
            _ => None,
        }
    }

    async fn send(&self, channel: &Channel, event: &Event) -> Result<(), String> {
        let secret = self.secrets.resolve(channel.secret_ref()).await?;
        let link = self.link(event);
        let (url, body) = match channel {
            Channel::Webhook { .. } => (
                secret,
                json!({
                    "source": "socle",
                    "event": event.kind,
                    "environment": event.environment,
                    "severity": event.severity.map(Severity::as_str),
                    "title": event.title,
                    "text": event.text,
                    "link": link,
                    "data": event.data,
                }),
            ),
            Channel::Slack { .. } => {
                let mut text = format!("*{}*", event.title);
                if !event.text.is_empty() {
                    text.push('\n');
                    text.push_str(&event.text);
                }
                if let Some(l) = &link {
                    text.push_str(&format!("\n<{l}|Ouvrir dans socle>"));
                }
                (secret, json!({ "text": text }))
            }
            Channel::Teams { .. } => {
                let mut card = json!({
                    "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
                    "type": "AdaptiveCard",
                    "version": "1.4",
                    "body": [
                        {"type": "TextBlock", "text": event.title, "weight": "Bolder", "size": "Medium", "wrap": true},
                        {"type": "TextBlock", "text": event.text, "wrap": true}
                    ]
                });
                if let Some(l) = &link {
                    card["actions"] = json!([{"type": "Action.OpenUrl", "title": "Ouvrir dans socle", "url": l}]);
                }
                (
                    secret,
                    json!({
                        "type": "message",
                        "attachments": [{"contentType": "application/vnd.microsoft.card.adaptive", "content": card}]
                    }),
                )
            }
            Channel::Pagerduty { .. } => {
                // PagerDuty n'est sollicité que pour les événements porteurs d'une gravité.
                let Some(sev) = event.severity else {
                    return Ok(());
                };
                let mut summary = event.title.clone();
                summary.truncate(1_000);
                (
                    "https://events.pagerduty.com/v2/enqueue".to_string(),
                    json!({
                        "routing_key": secret,
                        "event_action": if event.resolve { "resolve" } else { "trigger" },
                        "dedup_key": event.dedup_key,
                        "payload": {
                            "summary": summary,
                            "source": "socle",
                            "severity": sev.pagerduty(),
                            "group": event.environment,
                            "custom_details": event.data,
                        },
                        "links": link.map(|l| vec![json!({"href": l, "text": "socle"})]).unwrap_or_default(),
                    }),
                )
            }
        };
        self.http
            .post(&url)
            .json(&body)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map(|_| ())
            .map_err(|e| e.without_url().to_string())
    }
}
