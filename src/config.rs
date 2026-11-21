// SPDX-License-Identifier: AGPL-3.0-or-later
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub paths: Paths,
    pub corpus: Corpus,
    pub atl: Atl,
    pub display: Display,
    pub classify: Classify,
    pub agent: Option<Agent>,
    /// Per-app interaction scripts (src/interact.rs); optional.
    pub interact: Option<Interact>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interact {
    /// Directory with `<pkg>.txt` scripts (relative to the config file).
    pub dir: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paths {
    /// Corpus, logs, screenshots and the sqlite db (relative to the config file).
    pub work_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub index_url: String,
    pub repo_url: String,
    pub allowed_abis: Vec<String>,
    pub include_native: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Atl {
    /// `{apk}` and `{pkg}` are substituted in each element.
    pub command: Vec<String>,
    pub settle_s: u64,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Seed each run's ATL data dir (`$XDG_DATA_HOME/android_translation_layer`)
    /// from this directory, e.g. a microG install made with `--install-internal`.
    #[serde(default)]
    pub data_template: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Display {
    pub xvfb_args: Vec<String>,
    /// Rows to skip at the top of the screenshot when scoring blankness
    /// (ATL's own title bar, which is drawn even when the app shows nothing).
    #[serde(default)]
    pub crop_top_px: u32,
    /// Rows/columns skipped on every edge (ATL's light window frame), after crop_top_px.
    #[serde(default)]
    pub crop_border_px: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classify {
    pub stub_patterns: Vec<String>,
    pub blank_stddev_threshold: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    /// `{prompt}` = prompt text, `{prompt_file}` = path to it.
    #[allow(dead_code)] // consumed by the agent loop (docs/design.md stage 4), not built yet
    pub command: Vec<String>,
    pub atl_repo: PathBuf,
    pub aosp_frameworks: PathBuf,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let mut cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new("."));
        let work = base.join(&cfg.paths.work_dir);
        fs::create_dir_all(&work).with_context(|| format!("creating {}", work.display()))?;
        cfg.paths.work_dir = work.canonicalize()?;
        if let Some(i) = cfg.interact.as_mut() {
            i.dir = base.join(expand_tilde(&i.dir));
        }
        if let Some(t) = cfg.atl.data_template.as_mut() {
            *t = expand_tilde(t);
        }
        if let Some(agent) = cfg.agent.as_mut() {
            agent.atl_repo = expand_tilde(&agent.atl_repo);
            agent.aosp_frameworks = expand_tilde(&agent.aosp_frameworks);
        }
        Ok(cfg)
    }
}

pub fn expand_tilde(p: &Path) -> PathBuf {
    match (p.strip_prefix("~"), std::env::var_os("HOME")) {
        (Ok(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => p.to_path_buf(),
    }
}
