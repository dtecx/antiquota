//! Owns goal-mode decisions after a turn and after the check and verifier have spoken.
//! The application runs the check command and the verifier; this module only decides.
use antiquota_core::{
    DONE_MARK, Goal,
    goal::{VerifierVerdict, denied_note},
};

/// What an active goal needs after a turn.
#[derive(Debug, Clone, PartialEq)]
pub enum AfterTurn {
    /// Nothing more: the goal is not active, or retries are used up.
    Done,
    /// Background jobs are still running; the goal continues when they report.
    WaitForJobs { notice: String },
    /// Send this prompt as a nudge, first recording the notice.
    Nudge {
        notice: Option<String>,
        prompt: String,
    },
    /// The agent claims completion: run the optional check and verifier.
    Evaluate,
    /// The round limit is reached; the goal has failed.
    Abandon { notice: String },
    /// The turn failed; wait `delay` seconds, then nudge.
    Retry {
        delay: f64,
        notice: String,
        prompt: String,
    },
}

/// The check command's result, as the application observed it.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    pub command: String,
    /// `None` when the command timed out.
    pub code: Option<i32>,
    pub output: String,
}
impl CheckResult {
    pub fn passed(&self) -> bool {
        self.code == Some(0)
    }
    pub fn exit_text(&self) -> String {
        self.code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "timeout".into())
    }
}

/// The verifier's answer, if it ran.
#[derive(Debug, Clone, PartialEq)]
pub enum Verification {
    Skipped,
    /// No account could run it, or it gave no verdict.
    Unavailable,
    Verdict(VerifierVerdict),
}

/// Decides what an active goal does after a turn. `errors` counts consecutive failed turns
/// and is reset after a successful one. The round counter moves only on successful turns
/// that are not waiting for background jobs.
pub fn after_turn(
    goal: &mut Goal,
    ok: bool,
    reply: &str,
    waiting_jobs: &[String],
    errors: &mut u32,
) -> AfterTurn {
    if goal.state != "active" {
        return AfterTurn::Done;
    }
    let claimed = reply.contains(DONE_MARK);
    if ok && !waiting_jobs.is_empty() && !claimed {
        let more = if waiting_jobs.len() > 1 {
            format!(" and {} more", waiting_jobs.len() - 1)
        } else {
            String::new()
        };
        return AfterTurn::WaitForJobs {
            notice: format!(
                "Waiting for background job `{}`{more}; the goal continues when it finishes.",
                waiting_jobs[0]
            ),
        };
    }
    if ok {
        *errors = 0;
        goal.round += 1;
        if goal.round > goal.max_rounds {
            goal.state = "failed".into();
            return AfterTurn::Abandon {
                notice: format!("Goal abandoned after {} rounds.", goal.max_rounds),
            };
        }
        if !claimed {
            return AfterTurn::Nudge {
                notice: Some(format!(
                    "The agent stopped before claiming the goal is done. Pushing it on (round {}/{}).",
                    goal.round, goal.max_rounds
                )),
                prompt: format!(
                    "You stopped before the objective was achieved. Keep working toward it:\n{}\nEnd with {DONE_MARK} only when it is fully achieved.",
                    goal.objective
                ),
            };
        }
        return AfterTurn::Evaluate;
    }
    if *errors < 3 {
        *errors += 1;
        let delay = 30 * *errors;
        return AfterTurn::Retry {
            delay: f64::from(delay),
            notice: format!("The turn failed; retrying in {delay}s ({errors}/3)."),
            prompt: format!("Continue working toward the objective: {}", goal.objective),
        };
    }
    AfterTurn::Done
}

/// Whether the verifier still has to run: only when it is enabled and the check passed.
pub fn needs_verifier(goal: &Goal, check: Option<&CheckResult>) -> bool {
    goal.verifier && check.is_none_or(CheckResult::passed)
}

/// The nudge listing what is still wrong, or `None` when the goal is met (its state becomes
/// "met").
pub fn evaluate(
    goal: &mut Goal,
    check: Option<&CheckResult>,
    verification: &Verification,
) -> Option<String> {
    let mut problems = vec![];
    if let Some(check) = check.filter(|c| !c.passed()) {
        problems.push(format!(
            "The check command `{}` exited {}:\n{}",
            check.command,
            check.exit_text(),
            check.output
        ));
    }
    if problems.is_empty() {
        match verification {
            Verification::Skipped => {}
            Verification::Unavailable => problems.push(
                "The independent verifier could not run; re-check your work carefully.".into(),
            ),
            Verification::Verdict(v) if !v.done => {
                let missing: Vec<String> = v.missing.iter().map(|m| format!("- {m}")).collect();
                problems.push(format!(
                    "An independent verifier says the objective is not met: {}\n{}",
                    v.reason,
                    missing.join("\n")
                ));
            }
            Verification::Verdict(_) => {}
        }
    }
    if problems.is_empty() {
        goal.state = "met".into();
        return None;
    }
    Some(format!(
        "The objective is not met yet.\n\n{}\n\nFix this and continue. End with {DONE_MARK} only when everything passes.",
        problems.join("\n\n")
    ))
}

/// The bounded continuation after a successful non-goal turn whose commands were denied.
pub fn denied_follow_up(denied: &[String], used: &mut u32, allowance: u32) -> Option<String> {
    if denied.is_empty() || *used >= allowance {
        return None;
    }
    *used += 1;
    Some(format!("{}\n\nContinue the task.", denied_note(denied)))
}

/// Prefixes the denial note to a goal nudge, so agy learns both.
pub fn with_denials(prompt: String, denied: &[String]) -> String {
    if denied.is_empty() {
        prompt
    } else {
        format!("{}\n\n{prompt}", denied_note(denied))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn goal(max: u64) -> Goal {
        Goal {
            objective: "Ship it".into(),
            check: "true".into(),
            verifier: true,
            max_rounds: max,
            state: "active".into(),
            ..Default::default()
        }
    }
    #[test]
    fn rounds_move_only_on_successful_turns_and_stop_at_the_limit() {
        let mut g = goal(2);
        let mut errors = 2;
        assert!(matches!(
            after_turn(&mut g, true, "still going", &[], &mut errors),
            AfterTurn::Nudge { notice: Some(n), .. } if n.ends_with("(round 1/2).")
        ));
        assert_eq!((g.round, errors), (1, 0));
        assert_eq!(
            after_turn(&mut g, true, "done GOAL_COMPLETE", &[], &mut errors),
            AfterTurn::Evaluate
        );
        assert_eq!(
            after_turn(&mut g, true, "again", &[], &mut errors),
            AfterTurn::Abandon {
                notice: "Goal abandoned after 2 rounds.".into()
            }
        );
        assert_eq!(g.state, "failed");
        assert_eq!(
            after_turn(&mut g, true, "x", &[], &mut errors),
            AfterTurn::Done
        );
    }
    #[test]
    fn jobs_hold_the_goal_without_spending_a_round() {
        let mut g = goal(5);
        let mut errors = 0;
        let jobs = vec!["make train".to_owned(), "sleep 9".to_owned()];
        assert_eq!(
            after_turn(&mut g, true, "waiting", &jobs, &mut errors),
            AfterTurn::WaitForJobs {
                notice: "Waiting for background job `make train` and 1 more; the goal continues when it finishes.".into()
            }
        );
        assert_eq!(g.round, 0);
        assert_eq!(
            after_turn(&mut g, true, "GOAL_COMPLETE", &jobs, &mut errors),
            AfterTurn::Evaluate,
            "a completion claim is judged even with jobs running"
        );
    }
    #[test]
    fn failed_turns_retry_three_times_with_growing_waits() {
        let mut g = goal(5);
        let mut errors = 0;
        for (delay, n) in [(30.0, 1), (60.0, 2), (90.0, 3)] {
            let AfterTurn::Retry {
                delay: d,
                notice,
                prompt,
            } = after_turn(&mut g, false, "", &[], &mut errors)
            else {
                panic!("retry expected")
            };
            assert_eq!(d, delay);
            assert_eq!(
                notice,
                format!("The turn failed; retrying in {}s ({n}/3).", delay as u64)
            );
            assert_eq!(prompt, "Continue working toward the objective: Ship it");
        }
        assert_eq!(
            after_turn(&mut g, false, "", &[], &mut errors),
            AfterTurn::Done
        );
        assert_eq!(g.round, 0);
    }
    #[test]
    fn met_needs_the_check_and_the_verifier_to_agree() {
        let pass = CheckResult {
            command: "true".into(),
            code: Some(0),
            output: String::new(),
        };
        let fail = CheckResult {
            command: "make test".into(),
            code: Some(2),
            output: "1 failed".into(),
        };
        let timeout = CheckResult {
            code: None,
            ..fail.clone()
        };
        let yes = Verification::Verdict(VerifierVerdict {
            done: true,
            reason: "ok".into(),
            missing: vec![],
        });
        let no = Verification::Verdict(VerifierVerdict {
            done: false,
            reason: "Tests fail.".into(),
            missing: vec!["fix tests".into()],
        });
        let mut g = goal(5);
        assert!(!needs_verifier(&g, Some(&fail)));
        assert!(needs_verifier(&g, Some(&pass)));
        let text = evaluate(&mut g, Some(&fail), &Verification::Skipped).unwrap();
        assert!(text.contains("The check command `make test` exited 2:\n1 failed"));
        assert!(
            evaluate(&mut g, Some(&timeout), &Verification::Skipped)
                .unwrap()
                .contains("exited timeout")
        );
        let text = evaluate(&mut g, Some(&pass), &no).unwrap();
        assert!(text.contains("not met: Tests fail.\n- fix tests"));
        assert!(text.ends_with("End with GOAL_COMPLETE only when everything passes."));
        assert!(
            evaluate(&mut g, None, &Verification::Unavailable)
                .unwrap()
                .contains("could not run")
        );
        assert_eq!(g.state, "active");
        assert_eq!(evaluate(&mut g, Some(&pass), &yes), None);
        assert_eq!(g.state, "met");
        let mut unverified = Goal {
            verifier: false,
            ..goal(5)
        };
        assert!(!needs_verifier(&unverified, None));
        assert_eq!(
            evaluate(&mut unverified, None, &Verification::Skipped),
            None
        );
    }
    #[test]
    fn denials_get_a_bounded_follow_up() {
        let mut used = 0;
        let denied = vec!["rm x".to_owned()];
        assert!(denied_follow_up(&[], &mut used, 2).is_none());
        let first = denied_follow_up(&denied, &mut used, 2).unwrap();
        assert!(first.contains("`rm x`") && first.ends_with("\n\nContinue the task."));
        assert!(denied_follow_up(&denied, &mut used, 2).is_some());
        assert!(denied_follow_up(&denied, &mut used, 2).is_none());
        assert_eq!(with_denials("go".into(), &[]), "go");
        assert!(with_denials("go".into(), &denied).ends_with("`rm x`\n\ngo"));
    }
}
