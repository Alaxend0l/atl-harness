//! Run one APK under ATL in an isolated, headless X display.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use regex::Regex;

use crate::classify::{self, Failure, Kind};
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Still running at settle time, screen not blank.
    Alive,
    /// Still running, screen (nearly) uniform.
    Blank,
    /// Exited with status 0 before settle time.
    Exited,
    /// Non-zero exit or killed by a signal.
    Crash,
    /// Running, but the harness could not capture the screen.
    NoScreenshot,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Alive => "ALIVE",
            Outcome::Blank => "BLANK",
            Outcome::Exited => "EXITED",
            Outcome::Crash => "CRASH",
            Outcome::NoScreenshot => "NO_SCREENSHOT",
        }
    }

    /// Ordering used by the regression gate: higher is better.
    pub fn score(self) -> u8 {
        match self {
            Outcome::Alive => 3,
            Outcome::Blank | Outcome::Exited | Outcome::NoScreenshot => 1,
            Outcome::Crash => 0,
        }
    }
}

impl FromStr for Outcome {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "ALIVE" => Outcome::Alive,
            "BLANK" => Outcome::Blank,
            "EXITED" => Outcome::Exited,
            "CRASH" => Outcome::Crash,
            "NO_SCREENSHOT" => Outcome::NoScreenshot,
            other => bail!("unknown outcome {other:?}"),
        })
    }
}

#[derive(Debug)]
pub struct RunResult {
    pub outcome: Outcome,
    /// Exit code, or the negated signal number if killed by a signal.
    pub exit_code: Option<i32>,
    pub duration_s: f64,
    pub screen_stddev: Option<f64>,
    pub log_path: PathBuf,
    pub screenshot_path: Option<PathBuf>,
    pub failures: Vec<Failure>,
}

struct Xvfb {
    child: Child,
    display: String,
}

impl Xvfb {
    fn start(args: &[String]) -> Result<Self> {
        // -displayfd 1: Xvfb picks a free display and writes its number to stdout.
        let mut child = Command::new("Xvfb")
            .args(["-displayfd", "1", "-nolisten", "tcp"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("starting Xvfb (is it installed?)")?;
        let mut line = String::new();
        BufReader::new(child.stdout.take().expect("piped")).read_line(&mut line)?;
        let num = line.trim();
        if num.is_empty() {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Xvfb failed to start");
        }
        let display = format!(":{num}");
        Ok(Self { child, display })
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM) };
        if !matches!(
            wait_timeout(&mut self.child, Duration::from_secs(5)),
            Ok(Some(_))
        ) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn wait_timeout(child: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Terminate the app's whole process group (ATL may spawn helpers).
fn kill_group(child: &mut Child) {
    let pgid = child.id() as libc::pid_t;
    for sig in [libc::SIGTERM, libc::SIGKILL] {
        if unsafe { libc::killpg(pgid, sig) } != 0 {
            return; // group is gone
        }
        let _ = wait_timeout(child, Duration::from_secs(5));
        thread::sleep(Duration::from_millis(500));
    }
}

/// Capture the root window; return colour std-dev (0..1) as a blankness score.
/// The saved PNG is the full screen; the score skips the top `crop_top` rows.
fn screenshot(display: &str, dest: &Path, crop_top: u32) -> Result<f64> {
    let status = Command::new("import")
        .args(["-display", display, "-window", "root"])
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("running ImageMagick import")?;
    if !status.success() {
        bail!("import failed: {status}");
    }
    // `convert` works on ImageMagick 6 and 7 (7 prints a deprecation note).
    let out = Command::new("convert")
        .arg(dest)
        .arg("-chop")
        .arg(format!("0x{crop_top}"))
        .args(["-format", "%[fx:standard_deviation]", "info:"])
        .stderr(Stdio::null())
        .output()
        .context("running ImageMagick convert")?;
    if !out.status.success() {
        bail!("convert failed: {}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().parse()?)
}

fn signal_name(sig: i32) -> String {
    let name = match sig {
        libc::SIGSEGV => "SIGSEGV",
        libc::SIGABRT => "SIGABRT",
        libc::SIGBUS => "SIGBUS",
        libc::SIGILL => "SIGILL",
        libc::SIGFPE => "SIGFPE",
        libc::SIGKILL => "SIGKILL",
        libc::SIGTERM => "SIGTERM",
        libc::SIGTRAP => "SIGTRAP",
        _ => return format!("SIG{sig}"),
    };
    name.to_string()
}

pub fn run_one(
    cfg: &Config,
    stubs: &[Regex],
    pkg: &str,
    apk: &str,
    out_dir: &Path,
) -> Result<RunResult> {
    fs::create_dir_all(out_dir)?;
    let log_path = out_dir.join(format!("{pkg}.log"));
    let shot_path = out_dir.join(format!("{pkg}.png"));
    let cmd: Vec<String> = cfg
        .atl
        .command
        .iter()
        .map(|p| p.replace("{apk}", apk).replace("{pkg}", pkg))
        .collect();
    let (program, args) = cmd.split_first().context("[atl].command is empty")?;

    let home = tempfile::Builder::new().prefix("atlh-").tempdir()?;
    let h = home.path();
    // ATL won't create its default data dir (~/.local/share/...) itself.
    for d in [".local/share", ".config", ".cache"] {
        std::fs::create_dir_all(h.join(d))?;
    }
    let xvfb = Xvfb::start(&cfg.display.xvfb_args)?;
    let log = File::create(&log_path)?;

    let started = Instant::now();
    let mut child = Command::new(program)
        .args(args)
        .env_remove("WAYLAND_DISPLAY")
        .env("HOME", h)
        .env("XDG_DATA_HOME", h.join(".local/share"))
        .env("XDG_CONFIG_HOME", h.join(".config"))
        .env("XDG_CACHE_HOME", h.join(".cache"))
        .env("DISPLAY", &xvfb.display)
        .env("GDK_BACKEND", "x11")
        .envs(&cfg.atl.env)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .with_context(|| format!("spawning {program}"))?;

    let status = wait_timeout(&mut child, Duration::from_secs(cfg.atl.settle_s))?;
    let stddev = match status {
        Some(_) => None,
        None => screenshot(&xvfb.display, &shot_path, cfg.display.crop_top_px).ok(),
    };
    kill_group(&mut child);
    drop(xvfb);
    let duration_s = started.elapsed().as_secs_f64();

    let text = String::from_utf8_lossy(&fs::read(&log_path)?).into_owned();
    let mut failures = classify::parse(&text, stubs);
    let exit_code = status.map(|s| s.code().unwrap_or_else(|| -s.signal().unwrap_or(0)));

    let outcome = match (exit_code, stddev) {
        (Some(0), _) => Outcome::Exited,
        (Some(code), _) => {
            if code < 0 {
                failures.push(Failure {
                    kind: Kind::Signal,
                    signature: format!("signal: {}", signal_name(-code)),
                    detail: String::new(),
                });
            }
            Outcome::Crash
        }
        (None, None) => Outcome::NoScreenshot,
        (None, Some(sd)) if sd < cfg.classify.blank_stddev_threshold => Outcome::Blank,
        (None, Some(_)) => Outcome::Alive,
    };

    Ok(RunResult {
        outcome,
        exit_code,
        duration_s,
        screen_stddev: stddev,
        log_path,
        screenshot_path: shot_path.exists().then_some(shot_path),
        failures,
    })
}
