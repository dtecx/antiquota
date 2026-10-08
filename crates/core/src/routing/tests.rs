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
