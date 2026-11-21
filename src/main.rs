//! atlh — compatibility harness for a hard fork of Android Translation Layer.

mod classify;
mod config;
mod corpus;
mod db;
mod report;
mod runner;

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::{params, params_from_iter, Connection};

use config::Config;

#[derive(Parser)]
#[command(name = "atlh", about = "ATL compatibility harness")]
struct Cli {
    #[arg(long, default_value = "config.toml")]
    config: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download a random sample of F-Droid APKs into the corpus
    Fetch {
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long)]
        refresh_index: bool,
        /// Fetch exactly these packages instead of a random sample
        #[arg(long, num_args = 1..)]
        pkg: Vec<String>,
        /// Like --pkg, one package per line (# comments allowed)
        #[arg(long)]
        pkg_file: Option<PathBuf>,
    },
    /// Run corpus apps under ATL and record outcomes + failure signatures
    Run {
        /// Label for this run, e.g. the ATL commit: $(git -C ~/atl rev-parse --short HEAD)
        #[arg(long)]
        run_id: Option<String>,
        #[arg(long, default_value_t = 2)]
        jobs: usize,
        /// Max apps to run (0 = all)
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// Only these packages
        #[arg(long, num_args = 1..)]
        pkg: Vec<String>,
        /// Like --pkg, one package per line (# comments allowed)
        #[arg(long)]
        pkg_file: Option<PathBuf>,
        /// Rerun apps already recorded in this run
        #[arg(long)]
        force: bool,
        /// Seed each app's ATL data dir from this directory (overrides [atl].data_template)
        #[arg(long, conflicts_with = "no_data_template")]
        data_template: Option<PathBuf>,
        /// Ignore [atl].data_template for this run
        #[arg(long)]
        no_data_template: bool,
    },
    /// Rank failure signatures for a run by impact
    Rank {
        run_id: String,
        #[arg(long, default_value_t = 30)]
        top: usize,
    },
    /// Regression gate: exits 1 if any app got worse from BASE to NEW
    Compare { base: String, new: String },
    /// Write an agent prompt for the Nth ranked signature
    Workitem {
        run_id: String,
        #[arg(short, default_value_t = 1)]
        n: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load(&cli.config)?;
    let mut conn = db::connect(&cfg.paths.work_dir)?;
    match cli.cmd {
        Cmd::Fetch {
            limit,
            seed,
            refresh_index,
            pkg,
            pkg_file,
        } => {
            let only = with_pkg_file(pkg, pkg_file.as_deref())?;
            corpus::fetch(&cfg, &conn, limit, seed, refresh_index, &only)?;
        }
        Cmd::Run {
            run_id,
            jobs,
            limit,
            pkg,
            pkg_file,
            force,
            data_template,
            no_data_template,
        } => {
            let pkg = with_pkg_file(pkg, pkg_file.as_deref())?;
            let template = if no_data_template {
                None
            } else {
                data_template
                    .map(|p| config::expand_tilde(&p))
                    .or_else(|| cfg.atl.data_template.clone())
            };
            if let Some(t) = &template {
                anyhow::ensure!(
                    t.is_dir(),
                    "data template {} is not a directory",
                    t.display()
                );
            }
            let run_id = run_id.unwrap_or_else(|| {
                let secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                format!("run-{secs}")
            });
            let opts = RunOpts {
                jobs: jobs.max(1),
                limit,
                only: &pkg,
                force,
                template: template.as_deref(),
            };
            run(&cfg, &mut conn, &run_id, &opts)?;
        }
        Cmd::Rank { run_id, top } => report::rank(&conn, &run_id, top)?,
        Cmd::Compare { base, new } => {
            if report::compare(&conn, &base, &new)? > 0 {
                std::process::exit(1);
            }
        }
        Cmd::Workitem { run_id, n, out } => {
            let out =
                out.unwrap_or_else(|| cfg.paths.work_dir.join(format!("workitem-{run_id}-{n}.md")));
            report::workitem(&conn, &cfg, &run_id, n, &out)?;
            println!("{}", out.display());
        }
    }
    Ok(())
}

/// Packages from --pkg plus those listed in --pkg-file.
fn with_pkg_file(mut pkgs: Vec<String>, file: Option<&Path>) -> Result<Vec<String>> {
    if let Some(f) = file {
        let text =
            std::fs::read_to_string(f).with_context(|| format!("reading {}", f.display()))?;
        pkgs.extend(parse_pkg_list(&text));
    }
    Ok(pkgs)
}

fn parse_pkg_list(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

struct RunOpts<'a> {
    jobs: usize,
    limit: usize,
    only: &'a [String],
    force: bool,
    template: Option<&'a Path>,
}

fn run(cfg: &Config, conn: &mut Connection, run_id: &str, o: &RunOpts) -> Result<()> {
    let RunOpts {
        jobs,
        limit,
        only,
        force,
        template,
    } = *o;
    let mut sql = "SELECT pkg, apk_path FROM apps".to_string();
    if !only.is_empty() {
        sql += &format!(" WHERE pkg IN ({})", vec!["?"; only.len()].join(","));
    }
    sql += " ORDER BY pkg";
    let apps: Vec<(String, String)> = conn
        .prepare(&sql)?
        .query_map(params_from_iter(only), |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let done: HashSet<String> = conn
        .prepare("SELECT pkg FROM runs WHERE run_id=?1")?
        .query_map([run_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let todo: VecDeque<(String, String)> = apps
        .into_iter()
        .filter(|(pkg, _)| force || !done.contains(pkg))
        .take(if limit == 0 { usize::MAX } else { limit })
        .collect();
    let total = todo.len();
    let out_dir = cfg.paths.work_dir.join("runs").join(run_id);
    let stubs = classify::compile_patterns(&cfg.classify.stub_patterns)?;
    println!("run {run_id}: {total} apps, {jobs} jobs");
    if let Some(t) = template {
        println!("data template: {}", t.display());
    }

    let queue = Mutex::new(todo);
    let (tx, rx) = mpsc::channel();
    thread::scope(|s| -> Result<()> {
        for _ in 0..jobs {
            let tx = tx.clone();
            let (queue, stubs, out_dir) = (&queue, &stubs, &out_dir);
            s.spawn(move || loop {
                let Some((pkg, apk)) = queue.lock().unwrap().pop_front() else {
                    break;
                };
                let result = runner::run_one(cfg, stubs, &pkg, &apk, out_dir, template);
                if tx.send((pkg, result)).is_err() {
                    break;
                }
            });
        }
        drop(tx);

        for (i, (pkg, result)) in rx.iter().enumerate() {
            let r = match result {
                Ok(r) => r,
                Err(e) => {
                    println!("  [{}/{total}] {pkg}: HARNESS ERROR {e:#}", i + 1);
                    continue;
                }
            };
            let t = conn.transaction()?;
            t.execute(
                "DELETE FROM runs WHERE run_id=?1 AND pkg=?2",
                params![run_id, pkg],
            )?;
            t.execute(
                "INSERT INTO runs(run_id, pkg, outcome, exit_code, duration_s, screen_stddev, log_path, screenshot_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    run_id,
                    pkg,
                    r.outcome.as_str(),
                    r.exit_code,
                    r.duration_s,
                    r.screen_stddev,
                    r.log_path.to_string_lossy(),
                    r.screenshot_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                ],
            )?;
            let row = t.last_insert_rowid();
            for f in &r.failures {
                t.execute(
                    "INSERT INTO failures(run_row, kind, signature, detail) VALUES (?1, ?2, ?3, ?4)",
                    params![row, f.kind.as_str(), f.signature, f.detail],
                )?;
            }
            t.commit()?;
            println!(
                "  [{}/{total}] {pkg}: {} ({} signatures)",
                i + 1,
                r.outcome.as_str(),
                r.failures.len()
            );
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn pkg_list_parsing() {
        let text = "# microG apps\norg.tasks  # tasks\n\n  net.vonforst.evmap\n#gone\n";
        assert_eq!(
            super::parse_pkg_list(text),
            vec!["org.tasks", "net.vonforst.evmap"]
        );
    }
}
