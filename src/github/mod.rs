use std::{fs, path::Path};

use anyhow::Context;

use crate::domain::Resource;

pub mod api;
mod auth;
mod transport;

pub fn load_fixture(path: &Path) -> anyhow::Result<Resource> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read fixture {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse fixture {}", path.display()))
}
