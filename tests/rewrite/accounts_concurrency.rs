//! Proves exclusive account contexts and Python/Rust file-lock interoperability.
use crate::{setup, support::Server};
use serde_json::json;
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn concurrent_sessions_use_different_account_homes_and_skip_busy_probe() {
    let mut server = Server::new_stopped("concurrent-slots", true);
    setup::script(&server, json!({"delay_ms":1200}));
    setup::start(&mut server);
    let first = server.create();
    let second = server.create();
    setup::send(&server, &first, "First concurrent request.");
    setup::wait(&server, "first slot leased", |rows| rows[0]["busy"] == true);
    let busy = server.call(
        "POST",
        "/api/accounts/one@example.com/check",
        Some(json!({})),
    );
    assert_eq!(busy.status, 400);
    assert_eq!(
        busy.json(),
        json!({"error":"one@example.com is running a call; it is checked when that ends"})
    );
    setup::send(&server, &second, "Second concurrent request.");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if setup::turn_calls(&server).len() == 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "second session did not take another account"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let calls = setup::turn_calls(&server);
    assert_eq!(calls[0]["slot"], setup::ONE);
    assert_eq!(calls[1]["slot"], setup::TWO);
    assert_ne!(calls[0]["home"], calls[1]["home"]);
    assert!(
        calls
            .iter()
            .all(|row| row["ssh_connection_present"] == true)
    );
    assert!(calls.iter().all(|row| {
        row["home"]
            .as_str()
            .unwrap()
            .starts_with(server.data.join("slots").to_str().unwrap())
    }));
    server.idle(&first);
    server.idle(&second);
    setup::wait(&server, "leases released", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    setup::evidence(
        "concurrent-slots",
        json!({"slots":[setup::ONE,setup::TWO],"privateHomesDifferent":true,"busyProbeRejected":true,"leasesReleased":true}),
    );
    server.stop();
}

#[test]
fn python_account_lock_excludes_rust_probe_and_rust_lock_excludes_python() {
    let mut server = Server::new_stopped("python-lock", true);
    setup::start(&mut server);
    let lock = server.data.join("state/leases/one@example.com.lock");
    let script = if cfg!(windows) {
        "import os,sys,msvcrt\nf=os.open(sys.argv[1],os.O_RDWR|os.O_CREAT,0o600)\nmsvcrt.locking(f,msvcrt.LK_NBLCK,1)\nprint('locked',flush=True)\nsys.stdin.read()\n"
    } else {
        "import os,sys,fcntl\nf=os.open(sys.argv[1],os.O_RDWR|os.O_CREAT,0o600)\nfcntl.flock(f,fcntl.LOCK_EX|fcntl.LOCK_NB)\nprint('locked',flush=True)\nsys.stdin.read()\n"
    };
    let python = if cfg!(windows) { "python" } else { "python3" };
    let mut child = Command::new(python)
        .args(["-c", script, lock.to_str().unwrap()])
        .current_dir(&server.project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ready = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    assert_eq!(ready.trim(), "locked");
    assert!(antiquota_store::lock_file(&lock).unwrap().is_none());
    assert_eq!(
        server.call("GET", "/api/accounts", None).json()[0]["busy"],
        true
    );
    assert_eq!(
        server
            .call(
                "POST",
                "/api/accounts/one@example.com/check",
                Some(json!({}))
            )
            .status,
        400
    );
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
    let rust = antiquota_store::lock_file(&lock).unwrap().unwrap();
    let output = Command::new(python)
        .args(["-c", script, lock.to_str().unwrap()])
        .current_dir(&server.project)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    drop(rust);
    assert_eq!(
        server
            .call(
                "POST",
                "/api/accounts/one@example.com/check",
                Some(json!({}))
            )
            .status,
        200
    );
    setup::evidence(
        "python-lock",
        json!({"pythonExcludesRust":true,"rustExcludesPython":true,"busyHttpProbeRejected":true,"platform":std::env::consts::OS}),
    );
    server.stop();
}
