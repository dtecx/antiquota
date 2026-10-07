//! Owns fresh account-related defaults and compatible validation.
use crate::{AccountService, AppError};
use antiquota_core::Settings;
use serde_json::json;
impl AccountService {
    pub fn save_defaults(&self, values: &serde_json::Value) -> Result<Settings, AppError> {
        let _writer = self
            .settings_writer
            .lock()
            .map_err(|_| AppError::Invalid("settings unavailable".into()))?;
        let mut defaults = self.defaults()?;
        let extra = defaults.extra.clone();
        defaults.extra.clear();
        let mut data =
            serde_json::to_value(&defaults).map_err(|e| AppError::Invalid(e.to_string()))?;
        if let Some(values) = values.as_object() {
            for (key, value) in values {
                if data.get(key).is_some() {
                    data[key] = json!(match value {
                        serde_json::Value::String(text) => text.clone(),
                        serde_json::Value::Bool(true) => "True".into(),
                        serde_json::Value::Bool(false) => "False".into(),
                        serde_json::Value::Null => "None".into(),
                        v => v.to_string(),
                    });
                }
            }
        }
        defaults = serde_json::from_value(data).map_err(|e| AppError::Invalid(e.to_string()))?;
        if !["perishable", "sticky", "round-robin"].contains(&defaults.rotation.as_str()) {
            return Err(AppError::Invalid(
                "rotation: perishable · sticky · round-robin".into(),
            ));
        }
        for (key, value) in [
            ("keep_awake", &defaults.keep_awake),
            ("worktree_default", &defaults.worktree_default),
        ] {
            if !["on", "off"].contains(&value.as_str()) {
                return Err(AppError::Invalid(format!("{key}: on or off")));
            }
        }
        for (key, value, low, high) in [
            ("reserve", &mut defaults.reserve, 0.0, 50.0),
            ("quota_poll", &mut defaults.quota_poll, 2.0, 240.0),
            (
                "approval_timeout",
                &mut defaults.approval_timeout,
                1.0,
                240.0,
            ),
            ("max_rounds", &mut defaults.max_rounds, 1.0, 500.0),
            ("auto_continue", &mut defaults.auto_continue, 0.0, 10.0),
        ] {
            let number = value
                .parse::<f64>()
                .map_err(|_| AppError::Invalid(format!("{key}: a number")))?;
            if !number.is_finite() || number < low || number > high {
                return Err(AppError::Invalid(format!(
                    "{key}: between {low} and {high}"
                )));
            }
            *value = number.to_string();
        }
        if !defaults.default_cwd.is_empty() {
            self.store
                .validate_cwd(&defaults.default_cwd)
                .map_err(|_| {
                    AppError::Invalid(format!("not a folder: {}", defaults.default_cwd))
                })?;
        }
        let mut saved = defaults.clone();
        saved.extra = extra;
        self.store.save_settings(&saved)?;
        Ok(defaults)
    }
}
