//! Owns the Rust route catalog and explicit pending parity routes.
use serde::Serialize;
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum RouteStatus {
    Implemented,
    Pending,
}
#[derive(Debug, Clone, Serialize, TS)]
pub struct RouteDefinition {
    pub method: String,
    pub path: String,
    pub status: RouteStatus,
}
pub fn catalog() -> Vec<RouteDefinition> {
    vec![
        RouteDefinition {
            method: "GET".into(),
            path: "/api/state".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/approvals".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/approvals/{aid}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/approvals/{aid}/answer".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/fork".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/search".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/accounts/{name}/pause".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/steer".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/engine".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/engine/log".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/shutdown".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/term".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/term/input".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/term/resize".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/term/kill".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/tree".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/git".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/git/diff".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/git/commit".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/git/discard".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/git/merge".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/git/remove".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "PATCH".into(),
            path: "/api/sessions/{sid}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "DELETE".into(),
            path: "/api/sessions/{sid}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/send".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/rewind".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/clear".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/jobs".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/jobs/{jid}/stop".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/jobs/{jid}/log".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/loop".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "DELETE".into(),
            path: "/api/sessions/{sid}/loop".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/state".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/inherit".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/shell".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/note".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/file".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/compact".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/files".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/stop".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/goal".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/goal/resume".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "DELETE".into(),
            path: "/api/sessions/{sid}/goal".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/usage".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/routing".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/accounts".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/accounts/probe".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/accounts/login".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/accounts/login/{lid}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/accounts/login/{lid}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "DELETE".into(),
            path: "/api/accounts/login/{lid}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/accounts/{name}/check".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "DELETE".into(),
            path: "/api/accounts/{name}".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/settings".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "PUT".into(),
            path: "/api/settings/defaults".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/settings/file".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "PUT".into(),
            path: "/api/settings/file".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/settings/{kind}/{name}".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "PUT".into(),
            path: "/api/settings/{kind}/{name}".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "DELETE".into(),
            path: "/api/settings/{kind}/{name}".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/settings/{kind}/{name}/enabled".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/settings/copy".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/stack".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/stack/reseed".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/permissions".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "PUT".into(),
            path: "/api/permissions".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/fs".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/health".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/events".into(),
            status: RouteStatus::Implemented,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/term/stream".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/transcribe".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "POST".into(),
            path: "/api/sessions/{sid}/attach".into(),
            status: RouteStatus::Pending,
        },
        RouteDefinition {
            method: "GET".into(),
            path: "/api/sessions/{sid}/attachments/{name}".into(),
            status: RouteStatus::Pending,
        },
    ]
}
