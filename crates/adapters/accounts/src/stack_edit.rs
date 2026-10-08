//! Edits only antiquota's stack; imports CLI/IDE resources without writing their roots.
use crate::{FileAccounts, farm, files::read_json};
use antiquota_core::{PortError, policy::clean_rules};
use antiquota_store::{io_error, write_atomic, write_json_atomic};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::Duration,
};

pub(crate) fn check(kind: &str, name: Option<&str>, source: Option<&str>) -> Result<(), PortError> {
    if !["skill", "plugin", "rule", "hook", "mcp"].contains(&kind) {
        return Err(PortError::Invalid(format!("unknown kind: {kind}")));
    }
    if let Some(src) = source
        && !(src == "cli" || src == "ide" && ["skill", "mcp"].contains(&kind))
    {
        return Err(PortError::Invalid(format!("{src} has no {kind}s")));
    }
    if let Some(n) = name
        && (n.is_empty()
            || n.len() > 100
            || n.starts_with('.')
            || !n
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._:@+-".contains(&c)))
    {
        return Err(PortError::Invalid(format!("bad name: '{n}'")));
    }
    Ok(())
}
pub(crate) fn relative(kind: &str, source: &str) -> &'static str {
    match (kind, source) {
        ("skill", "ide") => "skills",
        ("skill", _) => "config/skills",
        ("plugin", _) => "config/plugins",
        ("rule", _) => "config/rules",
        ("hook", _) => "config/hooks.json",
        ("mcp", "ide") => "mcp_config.json",
        _ => "config/mcp_config.json",
    }
}
pub(crate) fn load(path: &Path) -> Value {
    read_json(path)
        .filter(Value::is_object)
        .unwrap_or(json!({}))
}
pub(crate) fn edit_object(path: &Path) -> Result<Value, PortError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => return Err(io_error(e)),
    };
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| PortError::Invalid(format!("existing stack JSON is invalid: {e}")))?;
    if !value.is_object() {
        return Err(PortError::Invalid(
            "existing stack JSON must be an object".into(),
        ));
    }
    Ok(value)
}
pub(crate) fn entries<'a>(kind: &str, value: &'a mut Value) -> &'a mut Value {
    if kind == "mcp" {
        if !value["mcpServers"].is_object() {
            value["mcpServers"] = json!({});
        }
        &mut value["mcpServers"]
    } else {
        value
    }
}
pub(crate) fn validate_permissions(value: &Value) -> Result<(), PortError> {
    if let Some(perms) = value.get("permissions") {
        for key in ["allow", "deny"] {
            if let Some(list) = perms.get(key) {
                clean_rules(key, list).map_err(PortError::Invalid)?;
            }
        }
    }
    Ok(())
}
impl FileAccounts {
    pub(crate) fn stack_probe(&self) -> Result<antiquota_core::Slot, PortError> {
        if let Some(slot) = self.slots()?.into_iter().next() {
            self.sync(&slot)?;
            return Ok(slot);
        }
        self.own()?;
        Ok(antiquota_core::Slot {
            name: "stack".into(),
            home: self.root.join("stack").to_string_lossy().into_owned(),
        })
    }
    pub(crate) fn own(&self) -> Result<PathBuf, PortError> {
        let path = self.root.join("stack/.gemini");
        if !path.is_dir() {
            self.seed_stack(false)?;
        }
        if farm::linked(&path)
            || farm::linked(&self.root.join("stack"))
            || farm::linked(&path.join("antigravity-cli"))
        {
            return Err(PortError::Invalid("own stack root cannot be linked".into()));
        }
        path.canonicalize().map_err(io_error)
    }
    pub(crate) fn editable(&self, rel: &str) -> Result<PathBuf, PortError> {
        let refuse = || PortError::Invalid(format!("not an editable stack file: {rel}"));
        let root = self.own()?;
        let relative = Path::new(rel);
        if relative.is_absolute()
            || relative.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::Prefix(_) | Component::RootDir
                )
            })
        {
            return Err(refuse());
        }
        let mut path = root.clone();
        for c in relative.components() {
            if let Component::Normal(n) = c {
                path.push(n);
                if path.is_symlink() {
                    return Err(refuse());
                }
            }
        }
        let parts: Vec<_> = relative
            .components()
            .filter_map(|c| {
                if let Component::Normal(n) = c {
                    n.to_str()
                } else {
                    None
                }
            })
            .collect();
        if parts.is_empty()
            || parts[0] == "antigravity-cli"
                && parts
                    .get(1)
                    .is_some_and(|n| farm::clone_skip("antigravity-cli", n) || farm::private(n))
        {
            return Err(refuse());
        }
        if path.exists() && !path.canonicalize().map_err(io_error)?.starts_with(&root) {
            return Err(refuse());
        }
        Ok(path)
    }
    pub(crate) fn read_stack_file(&self, rel: &str) -> Result<Value, PortError> {
        let path = self.editable(rel)?;
        if !path.is_file() {
            return Ok(json!({"path":rel,"text":"","exists":false}));
        }
        if fs::metadata(&path).map_err(io_error)?.len() > 1_000_000 {
            return Err(PortError::Invalid("file too large to edit here".into()));
        }
        Ok(
            json!({"path":rel,"text":String::from_utf8_lossy(&fs::read(path).map_err(io_error)?),"exists":true}),
        )
    }
    pub(crate) fn write_stack_text(&self, rel: &str, text: &str) -> Result<(), PortError> {
        let path = self.editable(rel)?;
        if text.chars().count() > 1_000_000 {
            return Err(PortError::Invalid("text too large".into()));
        }
        if path.extension().is_some_and(|e| e == "json") {
            let value: Value = serde_json::from_str(text).map_err(|e| {
                let reason = if e.is_eof() && text.trim_start().starts_with('{') {
                    "Expecting property name enclosed in double quotes"
                } else {
                    "Invalid JSON"
                };
                let column = e.column() + usize::from(e.is_eof());
                PortError::Invalid(format!(
                    "invalid JSON: {reason}: line {} column {column} (char {})",
                    e.line(),
                    column.saturating_sub(1)
                ))
            })?;
            if path == self.own()?.join("antigravity-cli/settings.json") {
                validate_permissions(&value)?;
            }
        }
        let _guard = self
            .changes
            .lock()
            .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
        write_atomic(&path, text.as_bytes())
    }
    pub(crate) fn get_stack_item(&self, kind: &str, name: &str) -> Result<Value, PortError> {
        check(kind, Some(name), None)?;
        if !["hook", "mcp"].contains(&kind) {
            return Err(PortError::Invalid(
                "only hooks and MCP servers are JSON items; edit files instead".into(),
            ));
        }
        let path = self.editable(relative(kind, "loop"))?;
        let mut data = edit_object(&path)?;
        entries(kind, &mut data)
            .get(name)
            .cloned()
            .ok_or_else(|| PortError::Invalid(format!("not found: '{name}'")))
    }
    pub(crate) fn put_stack_item(
        &self,
        kind: &str,
        name: &str,
        value: &Value,
    ) -> Result<(), PortError> {
        check(kind, Some(name), None)?;
        if !["hook", "mcp"].contains(&kind) || !value.is_object() {
            return Err(PortError::Invalid(
                "a hook or MCP server is a JSON object".into(),
            ));
        }
        let path = self.editable(relative(kind, "loop"))?;
        let _guard = self
            .changes
            .lock()
            .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
        let mut data = edit_object(&path)?;
        entries(kind, &mut data)[name] = value.clone();
        write_json_atomic(&path, &data)
    }
    pub(crate) fn delete_stack_item(&self, kind: &str, name: &str) -> Result<(), PortError> {
        check(kind, Some(name), None)?;
        let path = self.editable(relative(kind, "loop"))?;
        let _guard = self
            .changes
            .lock()
            .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
        if ["hook", "mcp"].contains(&kind) {
            let mut data = edit_object(&path)?;
            if entries(kind, &mut data)
                .as_object_mut()
                .unwrap()
                .remove(name)
                .is_none()
            {
                return Err(PortError::Invalid(format!("not found: '{name}'")));
            }
            write_json_atomic(&path, &data)
        } else {
            let target = path.join(name);
            if fs::symlink_metadata(&target).is_err() {
                return Err(PortError::Invalid(format!("not found: '{name}'")));
            }
            farm::remove_entry(&target)?;
            if kind == "plugin" {
                farm::remove_entry(
                    &self.editable(&format!("antigravity-cli/plugin_data/{name}"))?,
                )?;
            }
            Ok(())
        }
    }
    pub(crate) async fn enable_stack(
        &self,
        kind: &str,
        name: &str,
        on: bool,
    ) -> Result<(), PortError> {
        check(kind, Some(name), None)?;
        if kind == "hook" {
            let path = self.editable("config/hooks.json")?;
            let _guard = self
                .changes
                .lock()
                .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
            let mut data = edit_object(&path)?;
            let item = data
                .get_mut(name)
                .filter(|v| v.is_object())
                .ok_or_else(|| PortError::Invalid(format!("not found: '{name}'")))?;
            item["enabled"] = json!(on);
            return write_json_atomic(&path, &data);
        }
        if !["mcp", "plugin"].contains(&kind) {
            return Err(PortError::Invalid(format!(
                "{kind}s cannot be switched off; delete them instead"
            )));
        }
        self.own()?;
        let slot = self.stack_probe()?;
        let result = self
            .capture(
                &slot,
                &[
                    kind.into(),
                    if on { "enable" } else { "disable" }.into(),
                    name.into(),
                ],
                Duration::from_secs(60),
            )
            .await?;
        if result.code != 0 {
            return Err(PortError::Invalid(antiquota_core::state_document::tail(
                &format!("{}{}", result.out, result.err),
                300,
            )));
        }
        Ok(())
    }
    pub(crate) async fn stack_info_value(&self, reseed: bool) -> Result<Value, PortError> {
        self.seed_stack(reseed)?;
        let own = self.own()?;
        let slot = self.stack_probe()?;
        let mut result = json!({"path":own.to_string_lossy()});
        if self.slots()?.is_empty() {
            let skills = own.join("config/skills");
            let mut names: Vec<_> = fs::read_dir(skills)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            result["skills"] = json!(names);
            let mut hooks: Vec<_> = load(&own.join("config/hooks.json"))
                .as_object()
                .into_iter()
                .flatten()
                .map(|(name, _)| name.clone())
                .collect();
            hooks.sort();
            result["hooks"] = json!(hooks);
        } else {
            for (cmd, key) in [("/skills", "skills"), ("/hooks", "hooks")] {
                let response = self
                    .capture(
                        &slot,
                        &[
                            "-p".into(),
                            cmd.into(),
                            "--output-format".into(),
                            "json".into(),
                        ],
                        Duration::from_secs(90),
                    )
                    .await;
                result[key] = response
                    .ok()
                    .and_then(|r| serde_json::from_str::<Value>(&r.out).ok())
                    .and_then(|v| v["command"]["data"][key].as_array().cloned())
                    .map(|rows| {
                        let mut names: Vec<_> = rows
                            .into_iter()
                            .filter_map(|r| r["name"].as_str().map(str::to_owned))
                            .collect();
                        names.sort();
                        json!(names)
                    })
                    .unwrap_or(Value::Null);
            }
        }
        let mut names: Vec<_> = load(&own.join("config/mcp_config.json"))["mcpServers"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(n, _)| n.clone())
            .collect();
        names.sort();
        result["mcp"] = json!(names);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_kinds_and_import_sources_are_bounded() {
        for name in ["", ".hidden", "bad/name", "bad name", &"x".repeat(101)] {
            assert!(check("hook", Some(name), Some("cli")).is_err());
        }
        for name in ["A._:@+-1", &"x".repeat(100)] {
            assert!(check("hook", Some(name), Some("cli")).is_ok());
        }
        for kind in ["plugin", "rule", "hook"] {
            assert!(check(kind, Some("example"), Some("ide")).is_err());
        }
        for kind in ["skill", "mcp"] {
            assert!(check(kind, Some("example"), Some("ide")).is_ok());
        }
        assert!(check("unknown", None, None).is_err());
        assert!(check("skill", None, Some("unknown")).is_err());
    }
}
