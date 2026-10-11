use super::*;
use antiquota_core::{Goal, StepUsage};
use serde_json::json;

const ID: &str = "goal-session";
const CWD: &str = "/work/project";

fn meta(status: &str, goal: Option<&str>) -> Session {
    Session {
        id: ID.into(),
        status: status.into(),
        goal: goal.map(|state| Goal {
            state: state.into(),
            ..Goal::default()
        }),
        ..Session::default()
    }
}
fn record(ts: f64, event: Event) -> EventRecord {
    EventRecord::new(ts, event)
}
fn frame(sid: &str, record: EventRecord) -> String {
    let event = StreamEvent {
        sid: Some(sid.into()),
        record,
    };
    format!("data: {}\n\n", serde_json::to_string(&event).unwrap())
}
fn tool(ts: f64, state: &str, line: &str) -> EventRecord {
    record(
        ts,
        Event::Tool {
            turn: 1,
            step: 2,
            name: "run_command".into(),
            params: json!({ "CommandLine": line }),
            state: state.into(),
            secs: None,
            output: None,
            diff: None,
            error: None,
        },
    )
}
fn turn_start(ts: f64) -> EventRecord {
    record(
        ts,
        Event::TurnStart {
            turn: 1,
            slot: "one@example.com".into(),
            model: "gemini-3.8-flash-high".into(),
        },
    )
}
fn turn_end(ts: f64) -> EventRecord {
    record(
        ts,
        Event::TurnEnd {
            turn: 1,
            slot: "one@example.com".into(),
            status: "done".into(),
            usage: StepUsage::default(),
            secs: 2.31,
        },
    )
}
fn delta(ts: f64, text: &str) -> EventRecord {
    record(
        ts,
        Event::Delta {
            turn: 1,
            step: 3,
            text: text.into(),
        },
    )
}
fn reply(ts: f64, text: &str) -> EventRecord {
    record(
        ts,
        Event::Assistant {
            turn: 1,
            step: 3,
            text: text.into(),
        },
    )
}
fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap()
}

#[test]
fn the_log_shows_each_typed_event_as_the_legacy_goal_command_did() {
    let mut out = vec![];
    for event in [
        turn_start(1.0).event,
        tool(2.0, "running", "ls").event,
        tool(3.0, "done", "cat /work/project/notes.txt").event,
        tool(4.0, "error", "false").event,
        delta(5.0, "Hel").event,
        delta(5.1, "lo").event,
        reply(6.0, "Hello").event,
        Event::User {
            text: "a".repeat(500),
            kind: "goal".into(),
            attachments: None,
            steered: None,
        },
        Event::User {
            text: "typed by the user".into(),
            kind: "user".into(),
            attachments: None,
            steered: None,
        },
        Event::Notice {
            kind: "goal".into(),
            text: "check passed".into(),
            data: json!({}),
        },
        turn_end(7.0).event,
        Event::Deleted {},
    ] {
        show(&mut out, &event, CWD, true);
    }
    assert_eq!(
        text(out),
        format!(
            "\n-- turn 1 on one@example.com (gemini-3.8-flash-high)\n   . run_command cat notes.txt\n   x run_command false\nHello\n\n>> goal: {}\n== goal: check passed\n   done in 2.3s\n",
            "a".repeat(400)
        )
    );
    let mut saved = vec![];
    show(&mut saved, &reply(6.0, "Hello").event, CWD, false);
    assert_eq!(text(saved), "Hello\n", "a saved reply prints its own text");
}

#[test]
fn frames_decode_typed_events_for_this_session_and_report_unreadable_ones() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    let mut buffer = frame("other", turn_start(1.0)).into_bytes();
    buffer.extend(frame(ID, turn_start(1.0)).bytes());
    buffer.extend(b"data: {\"t\":\"turn_start\"}\n\n");
    buffer.extend(b": ping\n\n");
    let partial = frame(ID, turn_end(2.0)).into_bytes();
    buffer.extend(&partial[..20]);
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    assert_eq!(
        buffer,
        partial[..20],
        "a partial frame waits for the next chunk"
    );
    assert_eq!(
        text(out),
        "\n-- turn 1 on one@example.com (gemini-3.8-flash-high)\n"
    );
    let warning = text(warn);
    assert!(
        warning.starts_with("antiquota goal: skipped an unreadable event:"),
        "{warning}"
    );
    assert_eq!(warning.lines().count(), 1);
}

#[test]
fn the_run_ends_when_the_session_goes_idle_after_working() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    let meta_frame = |ts: f64, status: &str, goal: &str| {
        let mut value = meta(status, Some(goal));
        value.updated = ts;
        frame(
            ID,
            record(
                ts,
                Event::Meta {
                    meta: Box::new(value),
                },
            ),
        )
    };
    let mut buffer = meta_frame(1.0, "idle", "active").into_bytes();
    assert!(
        !follow.frames(&mut out, &mut warn, &mut buffer),
        "not started yet"
    );
    buffer = meta_frame(2.0, "running", "active").into_bytes();
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    buffer = meta_frame(3.0, "idle", "met").into_bytes();
    assert!(follow.frames(&mut out, &mut warn, &mut buffer));
    assert_eq!(follow.state, "met");
}

#[test]
fn catch_up_shows_only_the_missed_saved_events_once() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    let mut buffer = frame(ID, turn_start(1.0)).into_bytes();
    buffer.extend(frame(ID, tool(2.0, "done", "ls")).bytes());
    buffer.extend(frame(ID, delta(2.5, "Partly")).bytes());
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    // The engine ended the stream here; the saved session holds the rest of the turn.
    let saved = [
        turn_start(1.0),
        tool(2.0, "done", "ls"),
        reply(3.0, "Partly done."),
        turn_end(4.0),
    ];
    assert!(follow.catch_up(&mut out, &meta("idle", Some("met")), &saved));
    assert_eq!(follow.state, "met");
    // A late live copy of a saved event is not shown again.
    let mut late = frame(ID, turn_end(4.0)).into_bytes();
    assert!(!follow.frames(&mut out, &mut warn, &mut late));
    assert_eq!(
        text(out),
        "\n-- turn 1 on one@example.com (gemini-3.8-flash-high)\n   . run_command ls\nPartly done.\n   done in 2.3s\n"
    );
    assert!(warn.is_empty());
}

#[test]
fn catch_up_keeps_waiting_while_the_goal_is_still_active() {
    let mut follow = Follow::new(ID, CWD);
    let mut out = vec![];
    assert!(!follow.catch_up(&mut out, &meta("idle", Some("active")), &[]));
    assert!(!follow.catch_up(
        &mut out,
        &Session {
            updated: 1.0,
            ..meta("idle", None)
        },
        &[]
    ));
    assert!(follow.catch_up(
        &mut out,
        &Session {
            updated: 2.0,
            ..meta("idle", Some("abandoned"))
        },
        &[]
    ));
    assert!(out.is_empty());
}

#[test]
fn a_character_split_between_chunks_stays_whole() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    let bytes = frame(ID, delta(1.0, "naïve ✓")).into_bytes();
    let split = bytes.iter().position(|b| *b == 0xE2).unwrap() + 1;
    let mut buffer = bytes[..split].to_vec();
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    buffer.extend(&bytes[split..]);
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    assert_eq!(text(out), "naïve ✓");
    assert!(buffer.is_empty() && warn.is_empty());
}

fn metadata(ts: f64, status: &str, goal: &str) -> EventRecord {
    let mut value = meta(status, Some(goal));
    value.updated = ts;
    record(
        ts,
        Event::Meta {
            meta: Box::new(value),
        },
    )
}

#[test]
fn an_authoritative_snapshot_corrects_stale_completion_across_chunks() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    let mut current = meta("running", Some("active"));
    current.updated = 10.0;
    assert!(!follow.catch_up(&mut out, &current, &[]));
    for (older, candidate) in [
        (metadata(8.0, "idle", "paused"), true),
        (metadata(9.0, "idle", "active"), false),
    ] {
        let mut buffer = frame(ID, older).into_bytes();
        assert_eq!(follow.frames(&mut out, &mut warn, &mut buffer), candidate);
        assert!(!follow.catch_up(&mut out, &current, &[]));
        assert_eq!(follow.state, "active");
    }
    let mut buffer = frame(ID, metadata(11.0, "idle", "met")).into_bytes();
    assert!(follow.frames(&mut out, &mut warn, &mut buffer));
    assert_eq!(follow.state, "met");
    assert!(out.is_empty() && warn.is_empty());
}

#[test]
fn equal_or_backward_clock_metadata_can_finish_the_goal() {
    for updated in [10.0, 9.0] {
        let mut follow = Follow::new(ID, CWD);
        let (mut out, mut warn) = (vec![], vec![]);
        let mut current = meta("running", Some("active"));
        current.updated = 10.0;
        assert!(!follow.catch_up(&mut out, &current, &[]));
        let mut terminal = meta("idle", Some("met"));
        terminal.updated = updated;
        let mut buffer = frame(
            ID,
            record(
                20.0,
                Event::Meta {
                    meta: Box::new(terminal.clone()),
                },
            ),
        )
        .into_bytes();
        assert!(follow.frames(&mut out, &mut warn, &mut buffer));
        assert!(follow.catch_up(&mut out, &terminal, &[]));
        assert_eq!(follow.state, "met");
        assert!(out.is_empty() && warn.is_empty());
    }
}

#[test]
fn a_queued_resume_uses_the_newest_metadata_in_the_chunk() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    assert!(!follow.catch_up(&mut out, &meta("running", Some("active")), &[]));
    let mut buffer = frame(ID, metadata(1.0, "idle", "paused")).into_bytes();
    buffer.extend(frame(ID, metadata(2.0, "idle", "active")).bytes());
    buffer.extend(frame(ID, metadata(3.0, "waiting", "active")).bytes());
    buffer.extend(frame(ID, metadata(4.0, "running", "active")).bytes());
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    assert_eq!(follow.state, "active");
    let mut buffer = frame(ID, metadata(5.0, "idle", "active")).into_bytes();
    assert!(
        !follow.frames(&mut out, &mut warn, &mut buffer),
        "Resume can publish idle/active before waiting"
    );
    assert!(out.is_empty() && warn.is_empty());
}

#[test]
fn caught_up_replies_suppress_queued_deltas_and_keep_other_steps_separate() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    assert!(!follow.catch_up(
        &mut out,
        &meta("running", Some("active")),
        &[reply(3.0, "Fresh reply ✓")]
    ));
    let mut buffer = frame(ID, delta(2.0, "Fresh reply ✓")).into_bytes();
    buffer.extend(frame(ID, reply(3.0, "Fresh reply ✓")).bytes());
    buffer.extend(
        frame(
            ID,
            record(
                4.0,
                Event::Delta {
                    turn: 1,
                    step: 4,
                    text: "Other step".into(),
                },
            ),
        )
        .bytes(),
    );
    buffer.extend(
        frame(
            ID,
            record(
                5.0,
                Event::Assistant {
                    turn: 1,
                    step: 4,
                    text: "Other step done".into(),
                },
            ),
        )
        .bytes(),
    );
    buffer.extend(
        frame(
            ID,
            record(
                6.0,
                Event::Assistant {
                    turn: 2,
                    step: 3,
                    text: "Other turn".into(),
                },
            ),
        )
        .bytes(),
    );
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    assert_eq!(text(out), "Fresh reply ✓\nOther step done\nOther turn\n");
    assert!(warn.is_empty());
}

#[test]
fn caught_up_replies_append_only_the_missing_unicode_suffix() {
    let mut follow = Follow::new(ID, CWD);
    let (mut out, mut warn) = (vec![], vec![]);
    let mut buffer = frame(ID, delta(1.0, "naïve ✓")).into_bytes();
    assert!(!follow.frames(&mut out, &mut warn, &mut buffer));
    let events = [reply(2.0, "naïve ✓ done")];
    assert!(!follow.catch_up(&mut out, &meta("running", Some("active")), &events));
    assert!(!follow.catch_up(&mut out, &meta("running", Some("active")), &events));
    assert_eq!(text(out), "naïve ✓ done\n");
    assert!(warn.is_empty());
}

#[test]
fn resolved_approvals_do_not_reopen_from_queued_or_replayed_frames() {
    let mut f = Follow::new(ID, CWD);
    let approval = antiquota_core::Approval {
        id: "a1".into(),
        state: "pending".into(),
        ..Default::default()
    };
    let pending = record(
        1.0,
        Event::Approval {
            approval: approval.clone(),
        },
    );
    let resolved = record(
        2.0,
        Event::Approval {
            approval: antiquota_core::Approval {
                state: "allowed".into(),
                ..approval
            },
        },
    );
    let mut out = vec![];
    f.catch_up(
        &mut out,
        &meta("working", None),
        &[pending.clone(), resolved],
    );
    assert!(f.approvals.is_empty());
    let mut buffer = frame(ID, pending).into_bytes();
    f.frames(&mut out, &mut vec![], &mut buffer);
    assert!(f.approvals.is_empty());
    f.accepted();
    assert!(f.catch_up(&mut out, &meta("idle", None), &[]));
}

#[test]
fn the_interactive_prompt_is_echoed_once_across_stream_and_catch_up() {
    let mut f = Follow::new(ID, CWD);
    f.interactive = true;
    f.echoed_user = Some("same prompt".into());
    let user = record(
        1.0,
        Event::User {
            text: "same prompt".into(),
            kind: "user".into(),
            attachments: None,
            steered: None,
        },
    );
    let mut out = vec![];
    f.catch_up(
        &mut out,
        &meta("working", None),
        std::slice::from_ref(&user),
    );
    assert!(out.is_empty());
    assert!(f.echoed_user.is_none());
    f.catch_up(&mut out, &meta("idle", None), &[user]);
    assert!(out.is_empty());
    let user = record(
        2.0,
        Event::User {
            text: "same prompt".into(),
            kind: "user".into(),
            attachments: None,
            steered: None,
        },
    );
    f.catch_up(&mut out, &meta("working", None), &[user]);
    assert_eq!(String::from_utf8(out).unwrap(), "\nyou\nsame prompt\n");
}
