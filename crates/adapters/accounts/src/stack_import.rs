//! Imports supported CLI/IDE resources into the owned stack without changing source roots.
use crate::{
    FileAccounts, farm,
    stack_edit::{check, edit_object, entries, load, relative, validate_permissions},
};
use antiquota_core::PortError;
use antiquota_store::{io_error, write_json_atomic};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path};
impl FileAccounts {
    pub(crate) fn copy_stack(
        &self,
        source: &str,
        kind: &str,
        name: Option<&str>,
    ) -> Result<(), PortError> {
        if kind == "everything" {
            if source != "cli" {
                return Err(PortError::Invalid(
                    "everything comes from the agy CLI; copy IDE items one kind at a time".into(),
                ));
            }
            return self.seed_stack(true);
        }
        let own = self.own()?;
        let src = match source {
            "cli" => self.home.join(".gemini"),
            "ide" => self.home.join(".gemini/antigravity"),
            _ => return Err(PortError::Invalid(format!("{source} has no {kind}s"))),
        };
        if ["gemini_md", "permissions"].contains(&kind) {
            if source != "cli" {
                return Err(PortError::Invalid(format!("{kind} comes from the agy CLI")));
            }
            if kind == "gemini_md" {
                farm::replace(
                    &src.join("GEMINI.md"),
                    &own.join("GEMINI.md"),
                    "GEMINI.md",
                    &mut BTreeSet::new(),
                )?;
            } else {
                let path = self.editable("antigravity-cli/settings.json")?;
                let mut data = edit_object(&path)?;
                data["permissions"] =
                    load(&src.join("antigravity-cli/settings.json"))["permissions"].clone();
                validate_permissions(&data)?;
                write_json_atomic(&path, &data)?;
            }
            return Ok(());
        }
        check(kind, name, Some(source))?;
        let source_path = src.join(relative(kind, source));
        let target_path = self.editable(relative(kind, "loop"))?;
        let mut source_data = load(&source_path);
        let names: Vec<String> = if let Some(n) = name {
            vec![n.into()]
        } else if ["hook", "mcp"].contains(&kind) {
            entries(kind, &mut source_data)
                .as_object()
                .unwrap()
                .keys()
                .filter(|n| !n.starts_with('$'))
                .cloned()
                .collect()
        } else {
            fs::read_dir(&source_path)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    (!n.starts_with('.')
                        && (kind != "skill" || e.path().join("SKILL.md").is_file()))
                    .then_some(n)
                })
                .collect()
        };
        for n in &names {
            check(kind, Some(n), Some(source))?;
            if if ["hook", "mcp"].contains(&kind) {
                entries(kind, &mut source_data).get(n).is_none()
            } else {
                !source_path.join(n).exists()
            } {
                return Err(PortError::Invalid(format!("not found: '{n}'")));
            }
        }
        let _guard = self
            .changes
            .lock()
            .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
        if ["hook", "mcp"].contains(&kind) {
            let mut data = edit_object(&target_path)?;
            for n in &names {
                let rewritten =
                    farm::rewrite_paths(self, &entries(kind, &mut source_data)[n].to_string());
                entries(kind, &mut data)[n] = serde_json::from_str(&rewritten)
                    .map_err(|e| PortError::Invalid(e.to_string()))?;
                if kind == "mcp" {
                    let rel = if source == "cli" {
                        "antigravity-cli/mcp"
                    } else {
                        "mcp"
                    };
                    let state = src.join(rel).join(n);
                    let dst = self.editable(&format!("antigravity-cli/mcp/{n}"))?;
                    if state.is_dir() && !dst.exists() {
                        farm::replace(&state, &dst, "", &mut BTreeSet::new())?;
                    }
                }
            }
            write_json_atomic(&target_path, &data)?;
            if kind == "hook" && src.join("config/hooks").is_dir() {
                copy_scripts(&src.join("config/hooks"), &own.join("config/hooks"))?;
            }
        } else {
            for n in &names {
                farm::replace(
                    &source_path.join(n),
                    &target_path.join(n),
                    "",
                    &mut BTreeSet::new(),
                )?;
                if kind == "plugin" {
                    let data = src.join("antigravity-cli/plugin_data").join(n);
                    if data.is_dir() {
                        farm::replace(
                            &data,
                            &self.editable(&format!("antigravity-cli/plugin_data/{n}"))?,
                            "",
                            &mut BTreeSet::new(),
                        )?;
                    }
                    let cfg = load(&src.join("config/config.json"));
                    if let Some(entry) = cfg["plugins"].get(n) {
                        let path = self.editable("config/config.json")?;
                        let mut data = edit_object(&path)?;
                        if !data["plugins"].is_object() {
                            data["plugins"] = json!({});
                        }
                        data["plugins"][n] = entry.clone();
                        write_json_atomic(&path, &data)?;
                    }
                }
            }
        }
        Ok(())
    }
}
fn copy_scripts(src: &Path, dst: &Path) -> Result<(), PortError> {
    if dst.is_symlink() {
        return Err(PortError::Invalid("hook scripts cannot be linked".into()));
    }
    fs::create_dir_all(dst).map_err(io_error)?;
    for e in fs::read_dir(src).map_err(io_error)? {
        let e = e.map_err(io_error)?;
        let target = dst.join(e.file_name());
        if e.file_type().map_err(io_error)?.is_dir() {
            copy_scripts(&e.path(), &target)?;
        } else {
            farm::replace(&e.path(), &target, "", &mut BTreeSet::new())?;
        }
    }
    Ok(())
}
