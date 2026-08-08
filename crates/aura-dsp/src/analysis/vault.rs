//! Plugin config + preset scanner — shared across Aether, Equilibrium, Meridian.
//!
//! Moved from `lx-vault` (now deleted). Profile types live in individual plugins.

use serde::{Deserialize, Serialize};

// ─── Plugin Config ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PluginConfig {
    #[serde(default)]
    pub vault_path: Option<String>,
    /// Last preset the user selected — used as the default for fresh plugin
    /// instances (per-plugin, e.g. Aether). `serde(default)` keeps old
    /// config.json files (without this field) parseable.
    #[serde(default)]
    pub last_preset: Option<String>,
}

pub fn get_plugin_dir(plugin_name: &str) -> std::path::PathBuf {
    let mut path = if let Ok(appdata) = std::env::var("APPDATA") {
        std::path::PathBuf::from(appdata)
    } else if let Ok(home) = std::env::var("HOME") {
        let mut p = std::path::PathBuf::from(home);
        p.push(".config");
        p
    } else {
        std::path::PathBuf::from(".")
    };
    path.push(plugin_name);
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn load_config(plugin_name: &str) -> PluginConfig {
    let path = get_plugin_dir(plugin_name).join("config.json");
    if let Ok(content) = std::fs::read_to_string(path)
        && let Ok(config) = serde_json::from_str::<PluginConfig>(&content)
    {
        return config;
    }
    PluginConfig::default()
}

pub fn save_config(plugin_name: &str, config: &PluginConfig) -> Result<(), std::io::Error> {
    let path = get_plugin_dir(plugin_name).join("config.json");
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content)
}

// ─── Frontmatter parsing ─────────────────────────────────────────────────────

pub fn parse_frontmatter(content: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut lines = content.lines();

    if lines.next().map(|l| l.trim()) != Some("---") {
        return map;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if trimmed.starts_with("- ") {
            continue;
        }
        if let Some(pos) = trimmed.find(':') {
            let key = trimmed[..pos].trim().to_string();
            let val = trimmed[pos + 1..].trim().to_string();
            map.insert(key, val);
        }
    }
    map
}

/// Returns the `plugin:` field from frontmatter, or None if missing
pub fn preset_plugin_name(content: &str) -> Option<String> {
    parse_frontmatter(content).remove("plugin")
}
