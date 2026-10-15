//! Proves that the quota probe, version control and terminal ports can be replaced by fakes.
use super::*;
use antiquota_core::{
    Model, ProbeResult, Quota, Terminal, TerminalChunk, TerminalFeed, TerminalInfo, Terminals, Vcs,
    worktree::Worktree,
};
use std::{collections::VecDeque, sync::atomic::AtomicBool};

const ONE: &str = "one@example.com";

/// Answers each probe with the next scripted reading.
struct ScriptedProbe(Mutex<VecDeque<ProbeResult>>);
impl QuotaProbe for ScriptedProbe {
    fn probe<'a>(&'a self, _slot: &'a antiquota_core::Slot) -> PortFuture<'a, ProbeResult> {
        let next = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .expect("a scripted reading");
        Box::pin(async move { Ok(next) })
    }
    fn models<'a>(&'a self, _slot: &'a antiquota_core::Slot) -> PortFuture<'a, Vec<Model>> {
        Box::pin(async {
            Ok(vec![Model {
                id: antiquota_core::DEFAULT_MODEL.into(),
                label: "Gemini Flash High".into(),
            }])
        })
    }
}
/// One account whose every change is written to a ledger.
#[derive(Default)]
struct LedgerAccounts(Mutex<Vec<String>>);
impl LedgerAccounts {
    fn note(&self, line: String) -> Result<(), PortError> {
        self.0.lock().unwrap().push(line);
        Ok(())
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}
impl AccountStore for LedgerAccounts {
    fn accounts(&self) -> Result<Vec<antiquota_core::Account>, PortError> {
        Ok(vec![antiquota_core::Account {
            slot: ONE.into(),
            ..Default::default()
        }])
    }
    fn acquire(
        &self,
        _preferred: Option<&str>,
    ) -> Result<Option<Box<dyn antiquota_core::AccountLease>>, PortError> {
        Ok(Some(Box::new(Lease(antiquota_core::Slot {
            name: ONE.into(),
            home: "memory".into(),
        }))))
    }
    fn save_quota(&self, name: &str, quota: &Quota, now: f64) -> Result<(), PortError> {
        self.note(format!(
            "quota {name} {:?} at {now}",
            quota.0.keys().collect::<Vec<_>>()
        ))
    }
    fn set_wall(&self, name: &str, bucket: &str, until: f64, why: &str) -> Result<(), PortError> {
        self.note(format!("wall {name} {bucket} until {until}: {why}"))
    }
    fn clear_wall(&self, name: &str, bucket: &str) -> Result<(), PortError> {
        self.note(format!("clear {name} {bucket}"))
    }
    fn set_verify(&self, name: &str, url: Option<&str>, _now: f64) -> Result<(), PortError> {
        self.note(format!("verify {name} {url:?}"))
    }
    fn record_route(&self, entry: &antiquota_core::RoutingEntry) -> Result<(), PortError> {
        self.note(format!("route {} {:?}", entry.event, entry.why))
    }
    fn save_models(&self, models: &[Model]) -> Result<(), PortError> {
        self.note(format!("models {}", models[0].id))
    }
}

/// Records every version-control call; folders under `/repo` are a repository.
#[derive(Default)]
struct FakeVcs(Mutex<Vec<String>>);
impl FakeVcs {
    fn note(&self, line: String) {
        self.0.lock().unwrap().push(line);
    }
}
impl Vcs for FakeVcs {
    fn root(&self, cwd: &str) -> Option<String> {
        cwd.starts_with("/repo").then(|| "/repo".into())
    }
    fn add_worktree(
        &self,
        cwd: &str,
        branch: &str,
        folder: &str,
    ) -> Result<(Worktree, String), PortError> {
        self.note(format!("add {cwd} {branch}"));
        let worktree = Worktree {
            repo: "/repo".into(),
            path: folder.into(),
            branch: branch.into(),
            base: "main".into(),
            base_sha: "base123".into(),
            extra: BTreeMap::new(),
        };
        Ok((worktree, format!("{folder}/sub")))
    }
    fn status(
        &self,
        cwd: &str,
        worktree: Option<&Worktree>,
    ) -> Result<Option<serde_json::Value>, PortError> {
        self.note(format!("status {}", worktree.is_some()));
        Ok(Some(serde_json::json!({ "branch": "main", "cwd": cwd })))
    }
    fn diff(
        &self,
        _cwd: &str,
        path: Option<&str>,
        committed: Option<&Worktree>,
    ) -> Result<String, PortError> {
        self.note(format!(
            "diff {path:?} against {:?}",
            committed.map(|w| &w.base)
        ));
        Ok("+added line".into())
    }
    fn commit(&self, _cwd: &str, message: &str) -> Result<String, PortError> {
        self.note(format!("commit {message}"));
        Ok("abc1234".into())
    }
    fn discard(&self, _cwd: &str, path: &str) -> Result<(), PortError> {
        self.note(format!("discard {path}"));
        Ok(())
    }
    fn merge(&self, worktree: &Worktree, message: &str) -> Result<String, PortError> {
        self.note(format!("merge {} {message}", worktree.branch));
        Ok("def5678".into())
    }
    fn remove_worktree(&self, worktree: &Worktree, delete_branch: bool) -> Result<(), PortError> {
        self.note(format!("remove {} {delete_branch}", worktree.branch));
        Ok(())
    }
    fn rename_branch(&self, _path: &str, from: &str, to: &str) -> bool {
        self.note(format!("rename {from} {to}"));
        true
    }
}

/// A shell that keeps what it was sent and how large it is.
struct FakeTerminal {
    cwd: String,
    alive: AtomicBool,
    input: Mutex<Vec<u8>>,
    size: Mutex<(u16, u16)>,
}
impl Terminal for FakeTerminal {
    fn info(&self) -> TerminalInfo {
        TerminalInfo {
            alive: self.alive.load(Ordering::SeqCst),
            shell: "fake-shell".into(),
            cwd: self.cwd.clone(),
        }
    }
    fn write(&self, data: &[u8]) -> Result<(), PortError> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(PortError::Invalid(
                "the shell has exited: restart it".into(),
            ));
        }
        self.input.lock().unwrap().extend_from_slice(data);
        Ok(())
    }
    fn resize(&self, cols: u16, rows: u16) -> Result<(), PortError> {
        *self.size.lock().unwrap() = (cols, rows);
        Ok(())
    }
    fn kill(&self) {
        self.alive.store(false, Ordering::SeqCst);
    }
    fn subscribe(&self) -> TerminalFeed {
        let (tx, rx) = std::sync::mpsc::sync_channel(4);
        tx.send(TerminalChunk::Exit).unwrap();
        TerminalFeed {
            recent: self.input.lock().unwrap().clone(),
            chunks: rx,
        }
    }
}
#[derive(Default)]
struct FakeTerminals(Mutex<Vec<Arc<FakeTerminal>>>);
impl Terminals for FakeTerminals {
    fn spawn(&self, cwd: &str, cols: u16, rows: u16) -> Result<Arc<dyn Terminal>, PortError> {
        let terminal = Arc::new(FakeTerminal {
            cwd: cwd.into(),
            alive: AtomicBool::new(true),
            input: Mutex::new(vec![]),
            size: Mutex::new((cols, rows)),
        });
        self.0.lock().unwrap().push(terminal.clone());
        Ok(terminal)
    }
}

fn compose(
    accounts: Arc<dyn AccountStore>,
    quota: Option<Arc<dyn QuotaProbe>>,
    vcs: Option<Arc<dyn Vcs>>,
    terminals: Option<Arc<dyn Terminals>>,
) -> Arc<Service> {
    Service::compose(crate::Ports {
        store: Arc::new(MemoryStore::default()),
        settings: Settings::default(),
        accounts,
        runner: Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        clock: Arc::new(FixedClock),
        quota,
        shell: None,
        jobs: None,
        awake: None,
        vcs,
        workspace: None,
        terminals,
    })
    .unwrap()
}

#[tokio::test]
async fn a_scripted_quota_probe_drives_readings_walls_verification_and_sign_out() {
    let spent = Quota(BTreeMap::from([
        (
            "gemini".into(),
            BTreeMap::from([("weekly".into(), (0.0, Some(500.0)))]),
        ),
        (
            "cg".into(),
            BTreeMap::from([("weekly".into(), (0.8, Some(900.0)))]),
        ),
    ]));
    let probe = Arc::new(ScriptedProbe(Mutex::new(VecDeque::from([
        ProbeResult::Quota(spent),
        ProbeResult::Verify("https://accounts.google.com/signin/continue?fixture=1".into()),
        ProbeResult::SignedOut,
        ProbeResult::Unreadable,
        ProbeResult::Quota(Quota::default()),
    ]))));
    let ledger = Arc::new(LedgerAccounts::default());
    let service = compose(ledger.clone(), Some(probe.clone()), None, None);
    let accounts = &service.account_service;
    accounts.check_account(ONE).await.unwrap();
    assert_eq!(
        ledger.take(),
        [
            format!("quota {ONE} [\"cg\", \"gemini\"] at 10"),
            format!("verify {ONE} None"),
            format!("clear {ONE} cg"),
            format!("wall {ONE} gemini until 620: quota panel"),
        ]
    );
    accounts.check_account(ONE).await.unwrap();
    let parked = 10.0 + 30.0 * 86_400.0;
    assert_eq!(
        ledger.take(),
        [
            format!("verify {ONE} Some(\"https://accounts.google.com/signin/continue?fixture=1\")"),
            format!("wall {ONE} gemini until {parked}: verify the account with Google"),
            format!("wall {ONE} cg until {parked}: verify the account with Google"),
        ]
    );
    accounts.check_account(ONE).await.unwrap();
    assert_eq!(
        ledger.take(),
        [
            format!("wall {ONE} gemini until {parked}: signed out: sign in again"),
            format!("wall {ONE} cg until {parked}: signed out: sign in again"),
            "route signed-out Some(\"agy rejected the account's token\")".to_owned(),
        ]
    );
    accounts.check_account(ONE).await.unwrap();
    assert!(
        ledger.take().is_empty(),
        "an unreadable panel changes nothing"
    );
    // A refresh probes the stale account and fills the model list from the same fake.
    assert_eq!(accounts.refresh_accounts(0.0).await.unwrap(), [ONE]);
    assert_eq!(
        ledger.take(),
        [
            format!("quota {ONE} [] at 10"),
            format!("verify {ONE} None"),
            format!("models {}", antiquota_core::DEFAULT_MODEL),
        ]
    );
    assert!(probe.0.lock().unwrap().is_empty());
    assert!(matches!(
        accounts.check_account("missing@example.com").await,
        Err(AppError::Missing(_))
    ));
}

#[tokio::test]
async fn a_fake_vcs_carries_a_worktree_session_from_creation_to_removal() {
    let vcs = Arc::new(FakeVcs::default());
    let service = compose(Arc::new(Accounts), None, Some(vcs.clone()), None);
    assert!(matches!(
        service.create_in("/elsewhere", "", "model", antiquota_core::Mode::Default, true),
        Err(AppError::Invalid(text)) if text.contains("is not inside a git repository")
    ));
    let session = service
        .create_in(
            "/repo/sub",
            "Parser",
            "model",
            antiquota_core::Mode::Default,
            true,
        )
        .unwrap();
    let id = session.id.clone();
    let folder = std::path::Path::new("memory-worktrees")
        .join(format!("repo-{id}"))
        .to_string_lossy()
        .into_owned();
    assert_eq!(session.cwd, format!("{folder}/sub"));
    let branch = antiquota_core::worktree::branch_name("Parser", &id);
    assert_eq!(service.git_status(&id).unwrap()["branch"], "main");
    assert_eq!(
        service.git_diff(&id, Some("a.rs"), true).unwrap(),
        "+added line"
    );
    assert_eq!(
        service.git_commit(&id, "Add parser\n\nbody").unwrap(),
        "abc1234"
    );
    service.git_discard(&id, "b.rs").unwrap();
    assert_eq!(service.git_merge(&id).unwrap(), "def5678");
    let removed = service.git_remove_worktree(&id, true).unwrap();
    assert_eq!((removed.cwd.as_str(), removed.archived), ("/repo", true));
    assert!(removed.worktree.is_none() && removed.conversation_id.is_none());
    assert_eq!(
        *vcs.0.lock().unwrap(),
        [
            format!("add /repo/sub {branch}"),
            "status true".to_owned(),
            "diff Some(\"a.rs\") against Some(\"main\")".to_owned(),
            "commit Add parser\n\nbody".to_owned(),
            "discard b.rs".to_owned(),
            format!("merge {branch} Merge {branch}: Parser"),
            format!("remove {branch} true"),
        ]
    );
    let notices: Vec<String> = service
        .get(&id)
        .unwrap()
        .1
        .iter()
        .filter_map(|r| match &r.event {
            Event::Notice { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        notices,
        [
            "Committed abc1234: Add parser".to_owned(),
            "Discarded the changes to b.rs.".to_owned(),
            format!("Merged {branch} into main (def5678)."),
            "Removed the worktree and its branch; the session is archived.".to_owned(),
        ]
    );
}

#[tokio::test]
async fn fake_terminals_take_input_resize_restart_and_kill() {
    let terminals = Arc::new(FakeTerminals::default());
    let service = compose(Arc::new(Accounts), None, None, Some(terminals.clone()));
    let id = service
        .create("/work", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    let info = service.term_start(&id, 100, 30, false).unwrap();
    assert_eq!((info.alive, info.cwd.as_str()), (true, "/work"));
    service.term_start(&id, 100, 30, false).unwrap();
    assert_eq!(
        terminals.0.lock().unwrap().len(),
        1,
        "a live shell is reused"
    );
    service.term_input(&id, "ls\r").unwrap();
    service.term_resize(&id, 120, 40).unwrap();
    let first = terminals.0.lock().unwrap()[0].clone();
    assert_eq!(*first.input.lock().unwrap(), b"ls\r");
    assert_eq!(*first.size.lock().unwrap(), (120, 40));
    let (info, feed) = service.term_feed(&id).unwrap();
    assert!(info.alive);
    assert_eq!(feed.recent, b"ls\r", "a new viewer sees recent output");
    assert_eq!(feed.chunks.recv().unwrap(), TerminalChunk::Exit);
    // Restart ends the old shell and starts a fresh one in the same folder.
    service.term_start(&id, 80, 24, true).unwrap();
    let second = terminals.0.lock().unwrap()[1].clone();
    assert!(!first.alive.load(Ordering::SeqCst));
    assert_eq!(*second.size.lock().unwrap(), (80, 24));
    service.term_input(&id, "pwd\r").unwrap();
    assert_eq!(*second.input.lock().unwrap(), b"pwd\r");
    service.term_kill(&id);
    assert!(!second.alive.load(Ordering::SeqCst));
    assert!(matches!(
        service.term_input(&id, "x"),
        Err(AppError::Missing(_))
    ));
    service.term_resize(&id, 90, 20).unwrap();
    service.term_kill(&id);
    assert_eq!(terminals.0.lock().unwrap().len(), 2);
}
