//! Sample and download APKs from the F-Droid repo index (index-v2.json).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rusqlite::{params, Connection};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::Config;

#[derive(Deserialize)]
struct Index {
    #[serde(default)]
    packages: HashMap<String, Package>,
}

#[derive(Deserialize)]
struct Package {
    #[serde(default)]
    versions: HashMap<String, Version>,
}

#[derive(Deserialize)]
struct Version {
    file: Option<FileEntry>,
    #[serde(default)]
    manifest: Manifest,
}

#[derive(Deserialize)]
struct FileEntry {
    name: String,
    sha256: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(default)]
    version_code: i64,
    version_name: Option<String>,
    #[serde(default)]
    nativecode: Vec<String>,
    uses_sdk: Option<UsesSdk>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsesSdk {
    min_sdk_version: Option<i64>,
}

fn http() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(300))
        .user_agent("atl-harness/0.1")
        .build()
}

fn download(agent: &ureq::Agent, url: &str, dest: &Path, sha256: Option<&str>) -> Result<()> {
    let tmp = dest.with_extension("part");
    let mut reader = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_reader();
    let mut file = File::create(&tmp)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
    }
    drop(file);
    if let Some(expected) = sha256 {
        let got = format!("{:x}", hasher.finalize());
        if got != expected {
            fs::remove_file(&tmp)?;
            bail!("sha256 mismatch for {url}");
        }
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn fetch(
    cfg: &Config,
    conn: &Connection,
    limit: usize,
    seed: u64,
    refresh: bool,
) -> Result<usize> {
    let c = &cfg.corpus;
    let work = &cfg.paths.work_dir;
    let agent = http();
    let index_path = work.join("index-v2.json");
    if refresh || !index_path.exists() {
        println!("downloading index {} ...", c.index_url);
        download(&agent, &c.index_url, &index_path, None)?;
    }
    let index: Index = serde_json::from_reader(BufReader::new(File::open(&index_path)?))
        .context("parsing F-Droid index")?;

    let have: HashSet<String> = conn
        .prepare("SELECT pkg FROM apps")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let allowed: HashSet<&str> = c.allowed_abis.iter().map(String::as_str).collect();

    let mut pool: Vec<(String, Version)> = index
        .packages
        .into_iter()
        .filter(|(pkg, _)| !have.contains(pkg))
        .filter_map(|(pkg, p)| {
            let v = p
                .versions
                .into_values()
                .max_by_key(|v| v.manifest.version_code)?;
            v.file.as_ref()?;
            let abis = &v.manifest.nativecode;
            let ok = abis.is_empty()
                || (c.include_native && abis.iter().any(|a| allowed.contains(a.as_str())));
            ok.then_some((pkg, v))
        })
        .collect();
    pool.sort_by(|a, b| a.0.cmp(&b.0));
    pool.shuffle(&mut ChaCha8Rng::seed_from_u64(seed));
    println!("{} eligible apps not yet in corpus", pool.len());

    let apk_dir = work.join("apks");
    fs::create_dir_all(&apk_dir)?;
    let mut added = 0;
    for (pkg, v) in pool {
        if added >= limit {
            break;
        }
        let f = v.file.as_ref().expect("filtered above");
        let file_name = Path::new(&f.name).file_name().context("bad apk name")?;
        let dest = apk_dir.join(file_name);
        if !dest.exists() {
            let url = format!("{}{}", c.repo_url.trim_end_matches('/'), f.name);
            if let Err(e) = download(&agent, &url, &dest, f.sha256.as_deref()) {
                println!("  skip {pkg}: {e:#}");
                continue;
            }
        }
        let m = &v.manifest;
        conn.execute(
            "INSERT OR REPLACE INTO apps(pkg, version_code, version_name, apk_path, native_abis, min_sdk)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                pkg,
                m.version_code,
                m.version_name,
                dest.to_string_lossy(),
                serde_json::to_string(&m.nativecode)?,
                m.uses_sdk.as_ref().and_then(|u| u.min_sdk_version),
            ],
        )?;
        added += 1;
        println!("  [{added}/{limit}] {pkg}");
    }
    Ok(added)
}
