// Generated from Rust. Do not edit.
import type { RouteDefinition } from './RouteDefinition';
export const routes = [
  {
    "method": "GET",
    "path": "/api/state",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/approvals",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/approvals/{aid}",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/approvals/{aid}/answer",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/fork",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/search",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/accounts/{name}/pause",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/steer",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/engine",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/engine/log",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/shutdown",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/term",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/term/input",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/term/resize",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/term/kill",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/tree",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/git",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/git/diff",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/git/commit",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/git/discard",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/git/merge",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/git/remove",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}",
    "status": "implemented"
  },
  {
    "method": "PATCH",
    "path": "/api/sessions/{sid}",
    "status": "implemented"
  },
  {
    "method": "DELETE",
    "path": "/api/sessions/{sid}",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/send",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/rewind",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/clear",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/jobs",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/jobs/{jid}/stop",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/jobs/{jid}/log",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/loop",
    "status": "implemented"
  },
  {
    "method": "DELETE",
    "path": "/api/sessions/{sid}/loop",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/state",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/inherit",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/shell",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/note",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/file",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/compact",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/files",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/stop",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/goal",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/goal/resume",
    "status": "implemented"
  },
  {
    "method": "DELETE",
    "path": "/api/sessions/{sid}/goal",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/usage",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/routing",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/accounts",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/accounts/probe",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/accounts/login",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/accounts/login/{lid}",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/accounts/login/{lid}",
    "status": "implemented"
  },
  {
    "method": "DELETE",
    "path": "/api/accounts/login/{lid}",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/accounts/{name}/check",
    "status": "implemented"
  },
  {
    "method": "DELETE",
    "path": "/api/accounts/{name}",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/settings",
    "status": "implemented"
  },
  {
    "method": "PUT",
    "path": "/api/settings/defaults",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/settings/file",
    "status": "pending"
  },
  {
    "method": "PUT",
    "path": "/api/settings/file",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/settings/{kind}/{name}",
    "status": "pending"
  },
  {
    "method": "PUT",
    "path": "/api/settings/{kind}/{name}",
    "status": "pending"
  },
  {
    "method": "DELETE",
    "path": "/api/settings/{kind}/{name}",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/settings/{kind}/{name}/enabled",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/settings/copy",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/stack",
    "status": "pending"
  },
  {
    "method": "POST",
    "path": "/api/stack/reseed",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/permissions",
    "status": "implemented"
  },
  {
    "method": "PUT",
    "path": "/api/permissions",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/fs",
    "status": "pending"
  },
  {
    "method": "GET",
    "path": "/api/health",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/events",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/term/stream",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/transcribe",
    "status": "implemented"
  },
  {
    "method": "POST",
    "path": "/api/sessions/{sid}/attach",
    "status": "implemented"
  },
  {
    "method": "GET",
    "path": "/api/sessions/{sid}/attachments/{name}",
    "status": "implemented"
  }
] satisfies readonly RouteDefinition[];
