//! Owns goal prompts, the completion mark and the independent verifier's verdict format.
use crate::Goal;
use regex::Regex;
use std::sync::LazyLock;

/// The line agy ends its final message with once an objective is fully achieved.
pub const DONE_MARK: &str = "GOAL_COMPLETE";
pub const DENIED_NOTE: &str = "These commands were denied: headless agy cannot ask for permission, so the turn ended. Do not retry them or reword them; carry on without them:";
/// agy ignores a JSON schema for verifier calls, so the verifier ends with a plain verdict line.
static VERDICT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\W*VERDICT:\s*(DONE|NOT[_ ]DONE)\W*$").expect("valid verdict pattern")
});

pub fn goal_prompt(goal: &Goal) -> String {
    format!(
        "Objective: {}\n\nWork autonomously until the objective is fully achieved. Do not stop to ask for confirmation; make reasonable decisions yourself and keep going.\nWhen, and only when, the objective is fully achieved and verified, end your final message with the line {DONE_MARK}.",
        goal.objective
    )
}

/// The nudge that restarts a paused goal.
pub fn resume_prompt(goal: &Goal) -> String {
    format!(
        "Continue working toward the objective: {}\nEnd with {DONE_MARK} only when it is fully achieved.",
        goal.objective
    )
}

/// The read-only request for a fresh agy conversation that judges the objective.
pub fn verifier_prompt(objective: &str) -> String {
    format!(
        "You are a strict, independent verifier. Another agent claims this objective is complete:\n\n{objective}\n\nThe work is in the current directory. Inspect it yourself with file viewing and listing tools only: shell commands and edits are blocked here and would end your turn. Do not modify anything, do not plan fixes, and do not ask questions: only judge. Say in one or two sentences what you checked, list anything missing or broken as '- ' bullets, then end with exactly one final line: VERDICT: DONE if the objective is fully achieved, or VERDICT: NOT_DONE otherwise."
    )
}

pub fn denied_note(commands: &[String]) -> String {
    let list: Vec<String> = commands.iter().map(|c| format!("`{c}`")).collect();
    format!("{DENIED_NOTE} {}", list.join(", "))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifierVerdict {
    pub done: bool,
    pub reason: String,
    pub missing: Vec<String>,
}

/// The verdict of a verifier reply: the last VERDICT line decides, the text before it is the
/// reason (its last 1500 characters) and its `- `/`* ` bullets are what is missing (at most 20).
/// `None` when the reply has no verdict line.
pub fn parse_verdict(text: &str) -> Option<VerifierVerdict> {
    let hit = VERDICT.captures_iter(text).last()?;
    let whole = hit.get(0)?;
    let done = hit[1].eq_ignore_ascii_case("DONE");
    let body = text[..whole.start()].trim();
    let missing = if done {
        vec![]
    } else {
        body.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("- ") || line.starts_with("* "))
            .map(|line| line[2..].to_owned())
            .take(20)
            .collect()
    };
    let chars = body.chars().count();
    let reason: String = body.chars().skip(chars.saturating_sub(1500)).collect();
    let reason = if reason.is_empty() {
        if done {
            "Nothing missing."
        } else {
            "No reasons given."
        }
        .into()
    } else {
        reason
    };
    Some(VerifierVerdict {
        done,
        reason,
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verdicts_follow_the_last_line_and_list_what_is_missing() {
        let done = parse_verdict("Checked the files.\nVERDICT: DONE").unwrap();
        assert_eq!(
            (done.done, done.reason.as_str(), done.missing.len()),
            (true, "Checked the files.", 0)
        );
        let not = parse_verdict(
            "Looked.\n- tests are missing\n* docs\nnot a bullet\n**VERDICT: not done.**",
        )
        .unwrap();
        assert!(!not.done);
        assert_eq!(not.missing, ["tests are missing", "docs"]);
        let underscore = parse_verdict("x\nverdict: NOT_DONE").unwrap();
        assert!(!underscore.done);
        let last = parse_verdict("VERDICT: NOT_DONE\nlater fixed\nVERDICT: DONE").unwrap();
        assert!(last.done);
        assert_eq!(
            parse_verdict("VERDICT: DONE").unwrap().reason,
            "Nothing missing."
        );
        assert_eq!(
            parse_verdict("VERDICT: NOT DONE").unwrap().reason,
            "No reasons given."
        );
        assert!(parse_verdict("All good, I think").is_none());
        assert!(parse_verdict("The VERDICT: DONE line is inline").is_none());
        let many: String = (0..30).map(|i| format!("- item {i}\n")).collect();
        assert_eq!(
            parse_verdict(&format!("{many}VERDICT: NOT_DONE"))
                .unwrap()
                .missing
                .len(),
            20
        );
        let long = format!("{}\nVERDICT: DONE", "y".repeat(2000));
        assert_eq!(parse_verdict(&long).unwrap().reason.chars().count(), 1500);
    }
    #[test]
    fn prompts_name_the_objective_and_the_mark() {
        let goal = Goal {
            objective: "Ship it".into(),
            ..Default::default()
        };
        assert!(goal_prompt(&goal).starts_with("Objective: Ship it\n\n"));
        assert!(goal_prompt(&goal).ends_with("with the line GOAL_COMPLETE."));
        assert_eq!(
            resume_prompt(&goal),
            "Continue working toward the objective: Ship it\nEnd with GOAL_COMPLETE only when it is fully achieved."
        );
        assert!(verifier_prompt("Ship it").contains("\n\nShip it\n\n"));
        assert_eq!(
            denied_note(&["rm x".into(), "curl y".into()]),
            format!("{DENIED_NOTE} `rm x`, `curl y`")
        );
    }
}
