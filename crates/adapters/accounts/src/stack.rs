//! Owns read-only stack overview comparisons used by the settings account view.
use crate::{FileAccounts, farm::rewrite_paths, files::read_json};
use antiquota_core::PortError;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const EVENTS: [&str; 5] = [
    "PreToolUse",
    "PostToolUse",
    "PreInvocation",
    "PostInvocation",
    "Stop",
];
fn content(path: &Path) -> Option<BTreeMap<String, Vec<u8>>> {
    fn visit(
        root: &Path,
        path: &Path,
        result: &mut BTreeMap<String, Vec<u8>>,
        seen: &mut BTreeSet<std::path::PathBuf>,
    ) -> Option<()> {
        let canonical = path.canonicalize().ok()?;
        if !seen.insert(canonical.clone()) {
            return None;
        }
        if path.is_file() {
            result.insert(
                path.strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned(),
                fs::read(path).ok()?,
            );
        } else {
            for entry in fs::read_dir(path).ok()? {
                let p = entry.ok()?.path();
                if p.is_symlink() && p.is_dir() {
                    continue;
                }
                visit(root, &p, result, seen)?;
            }
        }
        seen.remove(&canonical);
        Some(())
    }
    if !path.exists() {
        return None;
    }
    let mut result = BTreeMap::new();
    visit(path, path, &mut result, &mut BTreeSet::new())?;
    Some(result)
}
fn items(
    accounts: &FileAccounts,
    root: &Path,
    kind: &str,
    source: &str,
) -> BTreeMap<String, Value> {
    let relative = match (kind, source) {
        ("skill", "ide") => "skills",
        ("skill", _) => "config/skills",
        ("plugin", _) => "config/plugins",
        ("rule", _) => "config/rules",
        ("hook", _) => "config/hooks.json",
        ("mcp", "ide") => "mcp_config.json",
        ("mcp", _) => "config/mcp_config.json",
        _ => return BTreeMap::new(),
    };
    let path = root.join(relative);
    if ["hook", "mcp"].contains(&kind) {
        let mut value = read_json(&path).unwrap_or(json!({}));
        if kind == "mcp" {
            value = value["mcpServers"].clone();
        }
        return value
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(name, v)| v.is_object() && !(kind == "hook" && name.starts_with('$')))
            .map(|(name, v)| {
                let encoded = if source == "loop" {
                    v.to_string()
                } else {
                    rewrite_paths(accounts, &v.to_string())
                };
                (name.clone(), json!(encoded))
            })
            .collect();
    }
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || kind == "skill" && !entry.path().join("SKILL.md").is_file()
            {
                return None;
            }
            content(&entry.path()).map(|bytes| (name, json!(bytes)))
        })
        .collect()
}
fn about(root: &Path, kind: &str, source: &str, name: &str) -> String {
    if kind == "skill" {
        let rel = if source == "ide" {
            "skills"
        } else {
            "config/skills"
        };
        let text =
            fs::read_to_string(root.join(rel).join(name).join("SKILL.md")).unwrap_or_default();
        return text
            .chars()
            .take(4000)
            .collect::<String>()
            .lines()
            .find_map(|line| {
                line.strip_prefix("description:").map(|text| {
                    text.trim()
                        .trim_matches(['\'', '"'])
                        .chars()
                        .take(200)
                        .collect()
                })
            })
            .unwrap_or_default();
    }
    if kind == "hook" {
        let value = read_json(&root.join("config/hooks.json")).unwrap_or_default();
        return EVENTS
            .into_iter()
            .filter(|event| value[name].get(*event).is_some())
            .collect::<Vec<_>>()
            .join(", ");
    }
    if kind == "mcp" {
        let rel = if source == "ide" {
            "mcp_config.json"
        } else {
            "config/mcp_config.json"
        };
        let value = read_json(&root.join(rel)).unwrap_or_default();
        let item = &value["mcpServers"][name];
        return item["url"]
            .as_str()
            .or_else(|| item["serverUrl"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                std::iter::once(item["command"].as_str().unwrap_or("").to_owned())
                    .chain(item["args"].as_array().into_iter().flatten().map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string())
                    }))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .into()
            });
    }
    String::new()
}
pub fn overview(accounts: &FileAccounts) -> Result<Value, PortError> {
    accounts.seed_stack(false)?;
    let own = accounts.root.join("stack/.gemini");
    let cli = accounts.home.join(".gemini");
    let ide = cli.join("antigravity");
    let roots = [("loop", own.clone()), ("cli", cli.clone()), ("ide", ide)];
    let mut result = json!({"path":own.to_string_lossy()});
    for kind in ["skill", "plugin", "rule", "hook", "mcp"] {
        let sources = if ["skill", "mcp"].contains(&kind) {
            vec!["cli", "ide"]
        } else {
            vec!["cli"]
        };
        let current = items(accounts, &own, kind, "loop");
        let mut others = BTreeMap::new();
        let mut names: BTreeSet<_> = current.keys().cloned().collect();
        for source in &sources {
            let root = &roots.iter().find(|r| r.0 == *source).unwrap().1;
            let map = items(accounts, root, kind, source);
            names.extend(map.keys().cloned());
            others.insert(*source, map);
        }
        let mut names: Vec<_> = names.into_iter().collect();
        names.sort_by_key(|n| n.to_lowercase());
        let mut rows = vec![];
        for name in names {
            let source = if current.contains_key(&name) {
                "loop"
            } else {
                *sources
                    .iter()
                    .find(|s| others[**s].contains_key(&name))
                    .unwrap()
            };
            let root = &roots.iter().find(|r| r.0 == source).unwrap().1;
            let mut row = json!({"name":name,"here":current.contains_key(&name),"about":about(root,kind,source,&name)});
            for source in &sources {
                row[*source] = match others[*source].get(&name) {
                    None => Value::Null,
                    Some(value) => json!(if current.get(&name) == Some(value) {
                        "same"
                    } else {
                        "differs"
                    }),
                };
            }
            if current.contains_key(&name) {
                if kind == "hook" {
                    row["enabled"] = json!(
                        read_json(&own.join("config/hooks.json")).unwrap_or_default()[&name]["enabled"]
                            != false
                    );
                }
                if kind == "plugin" {
                    row["enabled"] = json!(
                        read_json(&own.join("config/config.json")).unwrap_or_default()["plugins"]
                            [&name]["enabled"]
                            != false
                    );
                }
                if kind == "mcp" {
                    let state =
                        read_json(&own.join("antigravity-cli/mcp_state.json")).unwrap_or_default();
                    row["enabled"] = json!(state[&name].as_str() != Some("disabled"));
                }
            }
            rows.push(row);
        }
        result[format!("{kind}s")] = json!(rows);
    }
    let own_md = content(&own.join("GEMINI.md"));
    let cli_md = content(&cli.join("GEMINI.md"));
    result["gemini_md"] =
        json!({"cli":cli_md.as_ref().map(|v|if Some(v)==own_md.as_ref(){"same"}else{"differs"})});
    let permissions = |root: &Path| {
        read_json(&root.join("antigravity-cli/settings.json")).unwrap_or(json!({}))["permissions"]
            .as_object()
            .cloned()
            .unwrap_or_default()
    };
    let own_permissions = permissions(&own);
    let cli_permissions = permissions(&cli);
    result["permissions"] = json!({"allow":own_permissions.get("allow").cloned().unwrap_or(json!([])),"deny":own_permissions.get("deny").cloned().unwrap_or(json!([])),"cli":if own_permissions==cli_permissions{"same"}else{"differs"}});
    Ok(result)
}
