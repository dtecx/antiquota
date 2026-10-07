//! Owns plan mode by instruction: agy's own headless plan mode waits for a review nobody gives.

/// Prepended to a plan-mode request while antiquota's hook refuses edits.
pub const PLAN_NOTE: &str = "(antiquota plan mode) Investigate as needed, then reply with a concise, numbered plan for the request below. Do not edit files or run commands that change anything: edits are refused in this mode. The user approves the plan, and the next message asks you to carry it out.";
