//! Owns deterministic generation and stale checking of Rust API bindings.
use antiquota_core::{
    Approval, Command, Event, LoginLink, LoginState, RoutingEntry, Rule, Slot, StreamEvent, Turn,
};
use antiquota_server::{
    api::*,
    routes::{RouteDefinition, catalog},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use ts_rs::{Config, TS};
fn generate(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dir)?;
    let config = Config::from_env()
        .with_out_dir(dir)
        .with_large_int("number");
    StateResponse::export_all(&config)?;
    TextResponse::export_all(&config)?;
    StackFileResponse::export_all(&config)?;
    NoteResponse::export_all(&config)?;
    FilesResponse::export_all(&config)?;
    FolderResponse::export_all(&config)?;
    SearchHit::export_all(&config)?;
    UsageResponse::export_all(&config)?;
    StackResponse::export_all(&config)?;
    EngineResponse::export_all(&config)?;

    LoginLink::export_all(&config)?;
    LoginState::export_all(&config)?;
    LoginCodeRequest::export_all(&config)?;
    PauseAccountRequest::export_all(&config)?;
    SettingsOverview::export_all(&config)?;
    DefaultsRequest::export_all(&config)?;
    RoutingEntry::export_all(&config)?;
    SessionResponse::export_all(&config)?;
    CreateSessionRequest::export_all(&config)?;
    PatchSessionRequest::export_all(&config)?;
    SendRequest::export_all(&config)?;
    SendResponse::export_all(&config)?;
    OkResponse::export_all(&config)?;
    HealthResponse::export_all(&config)?;
    ApiError::export_all(&config)?;
    StreamEvent::export_all(&config)?;
    Command::export_all(&config)?;
    Event::export_all(&config)?;
    Approval::export_all(&config)?;
    antiquota_core::Settings::export_all(&config)?;
    Slot::export_all(&config)?;
    Rule::export_all(&config)?;
    Turn::export_all(&config)?;
    RouteDefinition::export_all(&config)?;
    ApprovalResponse::export_all(&config)?;
    ApprovalRequest::export_all(&config)?;
    ApprovalAnswerRequest::export_all(&config)?;
    SteerResponse::export_all(&config)?;
    AllowRulesResponse::export_all(&config)?;
    PermissionsRequest::export_all(&config)?;
    GoalRequest::export_all(&config)?;
    antiquota_core::TerminalInfo::export_all(&config)?;
    JobStartRequest::export_all(&config)?;
    LoopRequest::export_all(&config)?;
    JobLogResponse::export_all(&config)?;
    antiquota_core::jobs::Job::export_all(&config)?;
    antiquota_core::worktree::Worktree::export_all(&config)?;
    LoopResponse::export_all(&config)?;
    fs::write(
        dir.join("routes.ts"),
        format!(
            "// Generated from Rust. Do not edit.\nimport type {{ RouteDefinition }} from './RouteDefinition';\nexport const routes = {} satisfies readonly RouteDefinition[];\n",
            serde_json::to_string_pretty(&catalog())?
        ),
    )?;
    Ok(())
}
fn files(dir: &Path) -> std::io::Result<BTreeMap<String, Vec<u8>>> {
    fn visit(
        base: &Path,
        dir: &Path,
        output: &mut BTreeMap<String, Vec<u8>>,
    ) -> std::io::Result<()> {
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    visit(base, &path, output)?;
                } else if entry.file_type()?.is_file() {
                    output.insert(
                        path.strip_prefix(base)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .into_owned(),
                        fs::read(path)?,
                    );
                }
            }
        }
        Ok(())
    }
    let mut output = BTreeMap::new();
    visit(dir, dir, &mut output)?;
    Ok(output)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let default = root.join("ui/src/gen");
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|x| x == "--check") {
        let temporary = root.join("target/generated-types-check");
        if temporary.exists() {
            fs::remove_dir_all(&temporary)?;
        }
        generate(&temporary)?;
        let expected = files(&temporary)?;
        let actual = files(&default)?;
        fs::remove_dir_all(temporary)?;
        if expected != actual {
            return Err("generated UI types are stale; run just gen-types".into());
        }
    } else {
        generate(&args.first().map(PathBuf::from).unwrap_or(default))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recursive_stale_check_detects_changed_missing_and_extra_nested_files() {
        let owner = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
        let root = PathBuf::from(owner)
            .join("Desktop/test/antiquota-types")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let expected = root.join("expected");
        let actual = root.join("actual");
        generate(&expected).unwrap();
        generate(&actual).unwrap();
        let reference = files(&expected).unwrap();
        assert_eq!(reference, files(&actual).unwrap());
        let nested = actual.join("serde_json/JsonValue.ts");
        let original = fs::read(&nested).unwrap();
        fs::write(&nested, b"stale").unwrap();
        assert_ne!(reference, files(&actual).unwrap());
        fs::write(&nested, &original).unwrap();
        fs::remove_file(&nested).unwrap();
        assert_ne!(reference, files(&actual).unwrap());
        fs::write(&nested, original).unwrap();
        fs::write(actual.join("serde_json/Extra.ts"), b"extra").unwrap();
        assert_ne!(reference, files(&actual).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
