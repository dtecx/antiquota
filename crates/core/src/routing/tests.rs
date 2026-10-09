//! Proves routing order, reset boundaries and error-channel policy.
use super::*;
use proptest::prelude::*;
fn account(name: &str, fraction: f64, reset: f64) -> Account {
    Account {
        slot: name.into(),
        quota: Quota(
            [(
                "gemini".into(),
                [("weekly".into(), (fraction, Some(reset)))].into(),
            )]
            .into(),
        ),
        ..Default::default()
    }
}
#[test]
fn reserve_precedes_affinity_for_all_strategies() {
    let accounts = vec![
        account("preferred", 0.01, 10.0),
        account("healthy", 0.4, 20.0),
    ];
    for rotation in ["sticky", "round-robin", "perishable"] {
        let settings = Settings {
            rotation: rotation.into(),
            ..Default::default()
        };
        assert_eq!(
            rank_accounts(&accounts, "gemini", Some("preferred"), &settings, &[], 0.0),
            ["healthy", "preferred"]
        );
    }
}
#[test]
fn strategies_reset_padding_and_argument_forms_match_legacy() {
    let accounts = vec![account("a", 0.6, 50.0), account("b", 0.7, 20.0)];
    assert_eq!(
        rank_accounts(&accounts, "gemini", None, &Settings::default(), &[], 0.0),
        ["b", "a"]
    );
    assert_eq!(
        rank_accounts(
            &accounts,
            "gemini",
            Some("a"),
            &Settings::default(),
            &[],
            0.0
        ),
        ["a", "b"]
    );
    let settings = Settings {
        rotation: "round-robin".into(),
        ..Default::default()
    };
    let route = RoutingEntry {
        event: "pick".into(),
        slot: Some("a".into()),
        ts: 1.0,
        ..Default::default()
    };
    assert_eq!(
        rank_accounts(&accounts, "gemini", None, &settings, &[route], 0.0),
        ["b", "a"]
    );
    let q = account("a", 0.0, 300.0).quota;
    assert_eq!(quota_walls(&q, 0.0)["gemini"], Some(420.0));
    for args in [
        vec!["--model".into(), "claude-test".into()],
        vec!["--model=gpt-test".into()],
    ] {
        assert_eq!(bucket_of_args(&args), "cg");
    }
    assert_eq!(bucket_of_args(&["--model=gemini-test".into()]), "gemini");
    assert_eq!(classify_error("invalid_grant quota"), Some(Failure::Auth));
    assert_eq!(classify_error("429"), Some(Failure::Quota));
    assert_eq!(classify_error("503 overloaded"), Some(Failure::Transient));
}
proptest! {
    #[test] fn paused_or_walled_accounts_never_rank(paused in any::<bool>(), wall in 0.0f64..100000.0, now in 0.0f64..100000.0) {
        let mut row=account("one",0.4,1.0);row.paused=paused;row.walls.insert("gemini".into(),wall);
        let ranked=rank_accounts(&[row],"gemini",None,&Settings::default(),&[],now);
        prop_assert_eq!(!ranked.is_empty(),!paused && wall<=now);
    }
    #[test] fn ranking_is_deterministic_and_contains_no_duplicates(fractions in prop::collection::vec(0.0f64..1.0,0..30), now in 0.0f64..10000.0) {
        let rows:Vec<_>=fractions.iter().enumerate().map(|(i,f)|account(&format!("account-{i}"),*f,i as f64+1.0)).collect();
        let ranked=rank_accounts(&rows,"gemini",None,&Settings::default(),&[],now);let mut unique=ranked.clone();unique.sort();unique.dedup();
        prop_assert_eq!(unique.len(),rows.len());prop_assert_eq!(&ranked,&rank_accounts(&rows,"gemini",None,&Settings::default(),&[],now));
    }
}
#[test]
fn defaults_match_legacy_and_bad_numbers_fall_back() {
    let s = Settings::default();
    assert_eq!(
        [
            &s.model,
            &s.mode,
            &s.verifier_model,
            &s.rotation,
            &s.keep_awake,
            &s.reserve,
            &s.quota_poll,
            &s.approval_timeout,
            &s.max_rounds,
            &s.auto_continue,
            &s.default_cwd,
            &s.worktree_default,
        ]
        .map(String::as_str),
        [
            "gemini-3.8-flash-high",
            "default",
            "gemini-3.8-flash-high",
            "perishable",
            "on",
            "3",
            "10",
            "15",
            "20",
            "2",
            "",
            "off",
        ]
    );
    let with = |reserve: &str, quota_poll: &str| Settings {
        reserve: reserve.into(),
        quota_poll: quota_poll.into(),
        ..Settings::default()
    };
    assert_eq!(quota_fresh(&with("3", "abc")), 600.0);
    assert_eq!(quota_fresh(&with("3", "")), 600.0);
    assert_eq!(quota_fresh(&with("3", "5")), 300.0);
    // A 2% account sits under the default 3% reserve, so it ranks last with or without "abc".
    let accounts = [account("low", 0.02, 10.0), account("high", 0.5, 20.0)];
    for reserve in ["3", "abc"] {
        assert_eq!(
            rank_accounts(&accounts, "gemini", None, &with(reserve, "10"), &[], 0.0),
            ["high", "low"]
        );
    }
}
fn windows(bucket: &str, rows: &[(&str, f64, Option<f64>)]) -> Quota {
    Quota(
        [(
            bucket.into(),
            rows.iter()
                .map(|(name, fraction, reset)| ((*name).into(), (*fraction, *reset)))
                .collect(),
        )]
        .into(),
    )
}
fn quota_account(name: &str, quota: Quota) -> Account {
    Account {
        slot: name.into(),
        quota,
        ..Default::default()
    }
}
#[test]
fn models_classify_by_case_sensitive_prefix_from_either_argument_form() {
    for (model, bucket) in [
        ("claude-sonnet-fixture", "cg"),
        ("gpt-fixture", "cg"),
        ("claude", "cg"),
        ("gemini-3.8-flash-high", "gemini"),
        ("", "gemini"),
        ("Claude-fixture", "gemini"),
        ("GPT-fixture", "gemini"),
        ("my-claude", "gemini"),
        ("o-fixture", "gemini"),
    ] {
        assert_eq!(bucket_of_model(model), bucket, "{model}");
    }
    let args = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    for (argv, bucket) in [
        (args(&[]), "gemini"),
        (args(&["-p", "hello"]), "gemini"),
        (args(&["-p", "hello", "--model", "claude-fixture"]), "cg"),
        (args(&["--model=gpt-fixture", "-p", "hello"]), "cg"),
        (args(&["--model", "gemini-fixture"]), "gemini"),
        (args(&["--model=gemini-fixture"]), "gemini"),
        // As in legacy bucket_of, the separate form wins over the joined form.
        (
            args(&["--model=gemini-fixture", "--model", "claude-fixture"]),
            "cg",
        ),
        (
            args(&["--model=claude-fixture", "--model", "gemini-fixture"]),
            "gemini",
        ),
        // A trailing --model without a value leaves the joined form, or the default.
        (args(&["--model=gpt-fixture", "--model"]), "cg"),
        (args(&["--model"]), "gemini"),
    ] {
        assert_eq!(bucket_of_args(&argv), bucket, "{argv:?}");
    }
}
#[test]
fn walls_need_two_percent_or_less_and_use_the_latest_applicable_reset() {
    let now = 1_000.0;
    let walls = |rows: &[(&str, f64, Option<f64>)]| quota_walls(&windows("gemini", rows), now);
    // The threshold is inclusive at 2%.
    assert_eq!(
        walls(&[("5h", 0.02, Some(5_000.0))])["gemini"],
        Some(5_120.0)
    );
    assert_eq!(walls(&[("5h", 0.0201, Some(5_000.0))])["gemini"], None);
    assert_eq!(walls(&[("5h", 0.5, Some(5_000.0))])["gemini"], None);
    // A spent window's own reset applies, even when a healthy window resets later.
    assert_eq!(
        walls(&[("5h", 0.01, Some(2_000.0)), ("weekly", 0.6, Some(9_000.0))])["gemini"],
        Some(2_120.0)
    );
    // With two spent windows, the latest of their resets applies.
    assert_eq!(
        walls(&[("5h", 0.0, Some(2_000.0)), ("weekly", 0.02, Some(9_000.0))])["gemini"],
        Some(9_120.0)
    );
    assert_eq!(
        walls(&[("5h", 0.0, Some(9_000.0)), ("weekly", 0.01, Some(2_000.0))])["gemini"],
        Some(9_120.0)
    );
    // A spent window without a reset uses the latest reset of the other windows.
    assert_eq!(
        walls(&[
            ("5h", 0.0, None),
            ("weekly", 0.4, Some(7_000.0)),
            ("daily", 0.9, Some(3_000.0))
        ])["gemini"],
        Some(7_120.0)
    );
    // Unusable resets are ignored, and with no reset at all the fallback wall applies.
    assert_eq!(
        walls(&[("5h", 0.0, Some(f64::NAN)), ("weekly", 0.0, Some(0.0))])["gemini"],
        Some(now + FALLBACK_WALL + 120.0)
    );
    assert_eq!(
        walls(&[("5h", 0.0, None)])["gemini"],
        Some(now + FALLBACK_WALL + 120.0)
    );
    // Buckets are judged separately.
    let mut both = windows("gemini", &[("5h", 0.0, Some(4_000.0))]);
    both.0.insert(
        "cg".into(),
        [("5h".to_owned(), (0.9, Some(4_000.0)))].into(),
    );
    let walls = quota_walls(&both, now);
    assert_eq!(walls["gemini"], Some(4_120.0));
    assert_eq!(walls["cg"], None);
}
#[test]
fn headroom_is_the_smallest_fraction_and_the_latest_known_reset() {
    let quota = windows(
        "gemini",
        &[
            ("5h", 0.4, Some(100.0)),
            ("weekly", 0.7, Some(300.0)),
            ("daily", 0.9, None),
        ],
    );
    assert_eq!(headroom(&quota, "gemini"), (Some(0.4), Some(300.0)));
    // Missing quota stays unknown rather than empty or full.
    assert_eq!(headroom(&quota, "cg"), (None, None));
    assert_eq!(headroom(&Quota::default(), "gemini"), (None, None));
    assert_eq!(headroom(&windows("gemini", &[]), "gemini"), (None, None));
    // Known fractions without any reset keep the expiry unknown.
    assert_eq!(
        headroom(
            &windows("gemini", &[("5h", 0.3, None), ("weekly", 0.8, Some(0.0))]),
            "gemini"
        ),
        (Some(0.3), None)
    );
}
#[test]
fn perishable_spends_the_soonest_reset_then_more_headroom_with_unknown_expiry_last() {
    let rows = [
        quota_account("a-low", windows("gemini", &[("weekly", 0.3, Some(100.0))])),
        quota_account("b-high", windows("gemini", &[("weekly", 0.8, Some(100.0))])),
        quota_account("c-soon", windows("gemini", &[("weekly", 0.2, Some(50.0))])),
        quota_account("d-unknown", Quota::default()),
        quota_account("e-no-reset", windows("gemini", &[("weekly", 0.95, None)])),
        quota_account(
            "f-no-reset-low",
            windows("gemini", &[("weekly", 0.2, None)]),
        ),
    ];
    let settings = Settings::default();
    assert_eq!(settings.rotation, "perishable");
    assert_eq!(
        rank_accounts(&rows, "gemini", None, &settings, &[], 0.0),
        [
            "c-soon",
            "b-high",
            "a-low",
            "e-no-reset",
            "d-unknown",
            "f-no-reset-low"
        ]
    );
    // Input order never changes the result.
    let mut reversed = rows.to_vec();
    reversed.reverse();
    assert_eq!(
        rank_accounts(&reversed, "gemini", None, &settings, &[], 0.0),
        rank_accounts(&rows, "gemini", None, &settings, &[], 0.0)
    );
}
#[test]
fn sticky_uses_sorted_names_and_round_robin_the_least_recent_pick() {
    let rows = [
        quota_account("c", windows("gemini", &[("weekly", 0.9, Some(10.0))])),
        quota_account("a", windows("gemini", &[("weekly", 0.2, Some(900.0))])),
        quota_account("b", windows("gemini", &[("weekly", 0.5, Some(50.0))])),
        quota_account("d", Quota::default()),
    ];
    let pick = |slot: &str, ts: f64| RoutingEntry {
        event: "pick".into(),
        slot: Some(slot.into()),
        ts,
        ..Default::default()
    };
    let history = [
        pick("c", 5.0),
        pick("a", 10.0),
        pick("b", 1.0),
        pick("a", 2.0),
        // Other decisions never count as use.
        RoutingEntry {
            event: "wall".into(),
            slot: Some("d".into()),
            ts: 20.0,
            ..Default::default()
        },
    ];
    let sticky = Settings {
        rotation: "sticky".into(),
        ..Default::default()
    };
    assert_eq!(
        rank_accounts(&rows, "gemini", None, &sticky, &history, 30.0),
        ["a", "b", "c", "d"]
    );
    let round_robin = Settings {
        rotation: "round-robin".into(),
        ..Default::default()
    };
    // The last pick of each account counts: a was picked last at 2.
    assert_eq!(
        rank_accounts(&rows, "gemini", None, &round_robin, &history, 30.0),
        ["d", "b", "a", "c"]
    );
    // Accounts never picked tie at zero and fall back to name order.
    assert_eq!(
        rank_accounts(&rows, "gemini", None, &round_robin, &[], 30.0),
        ["a", "b", "c", "d"]
    );
}
#[test]
fn next_reset_is_the_earliest_live_wall_of_the_requested_bucket() {
    let walled = |name: &str, walls: &[(&str, f64)]| Account {
        slot: name.into(),
        walls: walls.iter().map(|(b, t)| ((*b).into(), *t)).collect(),
        ..Default::default()
    };
    let rows = [
        walled("a", &[("gemini", 500.0)]),
        walled("b", &[("gemini", 300.0), ("cg", 900.0)]),
        walled("c", &[("cg", 150.0)]),
        walled("d", &[("gemini", 50.0)]),
        walled("e", &[]),
    ];
    assert_eq!(next_reset(&rows, "gemini", 100.0), 300.0);
    assert_eq!(next_reset(&rows, "cg", 100.0), 150.0);
    // Expired walls are not resets, and no live wall reads as zero.
    assert_eq!(next_reset(&rows, "gemini", 600.0), 0.0);
    assert_eq!(next_reset(&rows[3..], "gemini", 100.0), 0.0);
}
#[test]
fn classification_prefers_auth_then_quota_then_transient() {
    use Failure::{Auth, Quota as Spent, Transient};
    for (text, expected) in [
        ("UNAUTHENTICATED", Some(Auth)),
        ("invalid_grant", Some(Auth)),
        ("token expired", Some(Auth)),
        ("Token has been revoked", Some(Auth)),
        ("Authentication required", Some(Auth)),
        ("You are not signed in", Some(Auth)),
        ("not logged in", Some(Auth)),
        ("please re-authenticate", Some(Auth)),
        ("reauthenticate", Some(Auth)),
        ("RESOURCE_EXHAUSTED", Some(Spent)),
        ("Individual quota reached", Some(Spent)),
        ("HTTP 429", Some(Spent)),
        ("rate limit", Some(Spent)),
        ("rate-limited", Some(Spent)),
        ("ratelimit", Some(Spent)),
        ("usage limit reached", Some(Spent)),
        ("UNAVAILABLE", Some(Transient)),
        ("the model is overloaded", Some(Transient)),
        ("no capacity", Some(Transient)),
        ("HTTP 500", Some(Transient)),
        ("502 bad gateway", Some(Transient)),
        ("503", Some(Transient)),
        ("504", Some(Transient)),
        ("DEADLINE_EXCEEDED", Some(Transient)),
        ("deadline exceeded", Some(Transient)),
        ("internal error", Some(Transient)),
        ("temporarily unavailable", Some(Transient)),
        ("connection reset by peer", Some(Transient)),
        ("connection refused", Some(Transient)),
        ("request timed out", Some(Transient)),
        ("timeout", Some(Transient)),
        ("please try again", Some(Transient)),
        // Precedence: an auth failure wins over quota and transient words, quota over transient.
        ("UNAUTHENTICATED after 429 and 503", Some(Auth)),
        ("503 then RESOURCE_EXHAUSTED", Some(Spent)),
        ("connection reset; quota left", Some(Spent)),
        // Codes must stand alone, and unrelated failures stay unclassified.
        ("error 4290", None),
        ("port 5000 closed", None),
        ("501 not implemented", None),
        ("permission denied", None),
        ("", None),
    ] {
        assert_eq!(classify_error(text), expected, "{text}");
    }
}
#[test]
fn freshness_uses_minutes_with_a_two_minute_floor_and_a_ten_minute_fallback() {
    let fresh = |text: &str| {
        quota_fresh(&Settings {
            quota_poll: text.into(),
            ..Default::default()
        })
    };
    for (text, seconds) in [
        ("10", 600.0),
        ("5", 300.0),
        ("2.5", 150.0),
        ("2", 120.0),
        ("1", 120.0),
        ("0", 120.0),
        ("-4", 120.0),
        ("240", 14_400.0),
        (" 5 ", 300.0),
        ("abc", 600.0),
        ("", 600.0),
        ("inf", 600.0),
        ("NaN", 600.0),
    ] {
        assert_eq!(fresh(text), seconds, "{text:?}");
    }
}
#[test]
fn error_text_keeps_agy_error_channels_without_the_reply() {
    use serde_json::json;
    let stream = [
        json!({"event":"init","conversation_id":"c"}),
        json!({"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"quota 429 UNAUTHENTICATED"}}),
        json!({"event":"result","result":{"status":"ERROR","error":"RESOURCE_EXHAUSTED","response":"UNAUTHENTICATED prose"}}),
    ]
    .map(|v| v.to_string())
    .join("\n");
    let text = error_text(1, &stream, "fatal: stderr line");
    assert!(text.starts_with("fatal: stderr line\n"));
    assert!(text.contains("RESOURCE_EXHAUSTED"));
    assert!(!text.contains("prose") && !text.contains("UNAUTHENTICATED"));
    assert_eq!(classify_error(&text), Some(Failure::Quota));
    // A successful call's stderr and replies are not error channels.
    let ok = json!({"status":"SUCCESS","response":"RESOURCE_EXHAUSTED quota"}).to_string();
    assert_eq!(error_text(0, &ok, "rate limit chatter"), "");
    // A single JSON document, as `--output-format json` prints it, is read whole.
    let failed = json!({"status":"ERROR","error":"tool crashed","response":"429"}).to_string();
    assert_eq!(classify_error(&error_text(0, &failed, "")), None);
    let frame = json!({"type":"error","message":"UNAVAILABLE"}).to_string();
    assert_eq!(
        classify_error(&error_text(0, &frame, "")),
        Some(Failure::Transient)
    );
}
