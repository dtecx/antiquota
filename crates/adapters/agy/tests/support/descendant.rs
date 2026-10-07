//! Provides a native synthetic agent whose descendants inherit both output pipes.
use std::{fs, io::Write, process::Command, thread, time::Duration};
fn frame(value: &str) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--descendant") {
        fs::write("descendant-ready", std::process::id().to_string()).unwrap();
        thread::sleep(Duration::from_secs(2));
        fs::write("descendant-wrote", "escaped runner lifetime").unwrap();
        thread::sleep(Duration::from_secs(60));
        return;
    }
    fs::write("runner-pid", std::process::id().to_string()).unwrap();
    fs::write("runner-args", args.join("\n")).unwrap();
    let prompt = args
        .windows(2)
        .find(|pair| pair[0] == "-p")
        .map(|pair| pair[1].as_str())
        .unwrap();
    frame(r#"{"event":"init","conversation_id":"early-resume-id"}"#);
    frame(
        r#"{"event":"step_update","step_update":{"step_index":1,"step_type":"tool","state":"DONE","tool_info":{"name":"read_file","parameters":{},"output":"durable before result"}}}"#,
    );
    if prompt == "resume" {
        frame(
            r#"{"event":"step_update","step_update":{"step_index":2,"step_type":"agent_response","state":"ACTIVE","text_delta":"resumed"}}"#,
        );
        frame(
            r#"{"event":"step_update","step_update":{"step_index":2,"step_type":"agent_response","state":"DONE"}}"#,
        );
        frame(
            r#"{"event":"result","result":{"status":"SUCCESS","conversation_id":"early-resume-id"}}"#,
        );
        return;
    }
    if prompt != "init-only" {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--descendant")
            .spawn()
            .unwrap();
        while !std::path::Path::new("descendant-ready").exists() {
            thread::sleep(Duration::from_millis(5));
        }
        if prompt == "crash" {
            std::process::exit(17);
        }
        if prompt == "success-with-descendant" {
            frame(
                r#"{"event":"result","result":{"status":"SUCCESS","conversation_id":"early-resume-id"}}"#,
            );
            return;
        }
        if prompt == "protocol-error" {
            print!("{}", "x".repeat(4_000_001));
            std::io::stdout().flush().unwrap();
        }
        let _ = child.wait();
    } else {
        thread::sleep(Duration::from_secs(60));
    }
}
