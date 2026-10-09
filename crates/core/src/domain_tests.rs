//! Proves that every named domain type survives a JSON round trip unchanged.
use crate::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Serializes, reads back and serializes again; both JSON forms must match.
fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> (T, Value) {
    let json = serde_json::to_value(value).unwrap();
    let back: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), json);
    (back, json)
}
fn same<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) -> Value {
    let (back, json) = round_trip(&value);
    assert_eq!(back, value);
    json
}

#[test]
fn sessions_turns_goals_and_events_round_trip() {
    let goal = Goal {
        objective: "Ship the parser".into(),
        check: "cargo test".into(),
        verifier: true,
        verifier_model: DEFAULT_MODEL.into(),
        max_rounds: 5,
        round: 2,
        state: "active".into(),
        extra: BTreeMap::from([("future".into(), json!({"kept": true}))]),
    };
    let session = Session {
        id: "s-1".into(),
        title: "Parser".into(),
        cwd: "/work/project".into(),
        status: "running".into(),
        turns: 3,
        goal: Some(goal),
        pinned: true,
        extra: BTreeMap::from([("unknown".into(), json!(12345678901234567890u64))]),
        ..Session::default()
    };
    let json = same(session);
    assert_eq!(json["goal"]["future"], json!({"kept": true}));
    assert_eq!(json["unknown"], json!(12345678901234567890u64));
    same(Turn {
        session: "s-1".into(),
        number: 4,
        slot: "one@example.com".into(),
        model: DEFAULT_MODEL.into(),
        status: "running".into(),
        retries: 1,
        failovers: 2,
    });
    let record = EventRecord::new(
        12.5,
        Event::Tool {
            turn: 1,
            step: 2,
            name: "run_command".into(),
            params: json!({"CommandLine": "ls"}),
            state: "done".into(),
            secs: Some(0.5),
            output: Some("a\nb".into()),
            diff: None,
            error: None,
        },
    );
    same(record.clone());
    let json = same(StreamEvent {
        sid: Some("s-1".into()),
        record,
    });
    assert_eq!(
        (json["t"].as_str(), json["sid"].as_str()),
        (Some("tool"), Some("s-1"))
    );
}

#[test]
fn commands_round_trip_in_every_form() {
    let commands = [
        Command::Create {
            cwd: "/work".into(),
            title: "New".into(),
            model: DEFAULT_MODEL.into(),
            mode: Mode::AcceptEdits,
        },
        Command::Patch {
            session: "s-1".into(),
            fields: SessionPatch {
                title: Some("Renamed".into()),
                pinned: Some(true),
                ..SessionPatch::default()
            },
        },
        Command::Send {
            session: "s-1".into(),
            text: "hello".into(),
            attachments: vec!["notes.txt".into()],
            kind: "user".into(),
        },
        Command::Stop {
            session: "s-1".into(),
        },
        Command::Delete {
            session: "s-1".into(),
        },
        Command::Account {
            action: AccountCommand::Pause {
                name: "one@example.com".into(),
                on: true,
            },
        },
        Command::ContinueTurn {
            session: "s-1".into(),
            delay: 1.5,
            fresh: false,
        },
        Command::SetAwake { on: true },
    ];
    let mut tags = vec![];
    for command in commands {
        let (_, json) = round_trip(&command);
        tags.push(json["command"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        tags,
        [
            "create",
            "patch",
            "send",
            "stop",
            "delete",
            "account",
            "continue_turn",
            "set_awake"
        ]
    );
}

#[test]
fn accounts_slots_quotas_approvals_rules_and_settings_round_trip() {
    let quota = Quota(BTreeMap::from([(
        "gemini".into(),
        BTreeMap::from([
            ("weekly".into(), (0.25, Some(3600.0))),
            ("daily".into(), (1.0, None)),
        ]),
    )]));
    same(quota.clone());
    same(Slot {
        name: "one@example.com".into(),
        home: "slots/one@example.com".into(),
    });
    same(Account {
        slot: "one@example.com".into(),
        busy: true,
        probed: Some(10.0),
        quota,
        walls: BTreeMap::from([("gemini".into(), 99.0)]),
        why: BTreeMap::from([("gemini".into(), "quota spent".into())]),
        paused: false,
        verify: None,
        today: TodayUsage {
            input: Some(100),
            turns: Some(1),
            ..TodayUsage::default()
        },
    });
    same(Approval {
        id: "a-1".into(),
        session: "s-1".into(),
        tool: "run_command".into(),
        args: json!({"CommandLine": "touch notes"}),
        rule: "command(touch)".into(),
        state: "pending".into(),
        why: String::new(),
        deadline: 70.0,
        extra: BTreeMap::new(),
    });
    same(Rule {
        text: "command(git status)".into(),
        deny: false,
    });
    let settings = Settings {
        rotation: "round-robin".into(),
        quota_poll: "5".into(),
        ..Settings::default()
    };
    let json = same(settings);
    assert_eq!(json["model"], DEFAULT_MODEL);
    assert_eq!(DEFAULT_MODEL, "gemini-3.8-flash-high");
}
