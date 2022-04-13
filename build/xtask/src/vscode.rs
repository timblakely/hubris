use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::Config;

#[derive(Debug, Deserialize, Serialize)]
struct Settings {
    #[serde(rename = "rust-analyzer.server.extraEnv")]
    env: serde_json::Map<String, serde_json::Value>,
    #[serde(flatten)]
    other: serde_json::Value,
}

const CONFIG_FILE: &str = ".vscode/settings.json";

pub fn hi(cfg: &Path) -> Result<()> {
    println!("Hi: {}", cfg.display());

    let toml = Config::from_file(&cfg)?;

    let mut out = PathBuf::from("target");
    out.push(&toml.name);
    out.push("dist");

    println!("Path: {}", out.to_str().unwrap());

    let settings = fs::read_to_string(CONFIG_FILE)?;

    let mut settings: Settings = serde_json::from_str(settings.as_str())?;

    let task_names = toml
        .tasks
        .iter()
        .map(|t| t.0.clone())
        .collect::<Vec<String>>()
        .join(",");

    println!("Tasks: {}", task_names);

    settings.env.insert(
        "HUBRIS_TASKS".to_string(),
        serde_json::Value::String(task_names),
    );

    let new_config = serde_json::to_string_pretty(&settings)?;
    println!("{}", new_config);

    fs::write(CONFIG_FILE, new_config).expect("Couldn't write file");

    Ok(())
}
