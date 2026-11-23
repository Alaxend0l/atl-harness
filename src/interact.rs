// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-app interaction scripts: drive an app on its Xvfb display after it has
//! settled (tap, type, keys), take screenshots and check what happened.
//!
//! Script: `<interact.dir>/<pkg>.txt`, one step per line, `#` comments:
//!   wait SECONDS          sleep (fractions allowed)
//!   tap X Y               click at screen pixels (the Xvfb screen, e.g. 1080x1920)
//!   long-press X Y [SECS] press and hold (default 1 s)
//!   type "TEXT"           type text into the focused widget
//!   key NAME              press a key (xdotool names: Return, BackSpace, Escape, ctrl+a;
//!                         semantic media names: media-play, media-pause, media-stop,
//!                         media-next, media-previous, media-rewind, media-fast-forward,
//!                         volume-up, volume-down, volume-mute — ATL maps these to the
//!                         Android media key codes; see docs/usage.md)
//!   shot NAME             screenshot to <run dir>/<pkg>.<NAME>.png
//!   expect-changed [PCT]  the latest screenshot differs from the one before it in
//!                         more than PCT % of pixels (default 0.005 %, ~100 px on
//!                         1080x1920: a new "15" or a typed word, not a blinking
//!                         caret); the settle screenshot counts as the first
//!   expect-text "TEXT"    OCR (tesseract) of the latest screenshot contains TEXT
//! After every step the app must still be running; if it exits, the script stops
//! and the run is classified from its exit like any crash.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Wait(f64),
    Tap(i32, i32),
    LongPress(i32, i32, f64),
    Type(String),
    Key(String),
    Shot(String),
    /// minimum fraction of changed pixels
    ExpectChanged(f64),
    ExpectText(String),
}

const DEFAULT_CHANGED: f64 = 0.000_05;

#[derive(Debug, Clone)]
pub struct StepResult {
    pub line: usize,
    pub text: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct Interaction {
    pub steps: Vec<StepResult>,
    /// The app exited during the script (after this many steps).
    pub exited: bool,
}

impl Interaction {
    pub fn passed(&self) -> bool {
        !self.exited && self.steps.iter().all(|s| s.ok)
    }

    pub fn first_failure(&self) -> Option<&StepResult> {
        self.steps.iter().find(|s| !s.ok)
    }
}

fn quoted(rest: &str, line: usize) -> Result<String> {
    let r = rest.trim();
    if r.len() >= 2 && r.starts_with('"') && r.ends_with('"') {
        Ok(r[1..r.len() - 1].replace("\\\"", "\"").replace("\\n", "\n"))
    } else {
        bail!("line {line}: expected a \"quoted\" string, got {r:?}")
    }
}

pub fn parse(text: &str) -> Result<Vec<(usize, Step)>> {
    let mut steps = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        // strip comments outside quotes
        let mut line = String::new();
        let mut in_q = false;
        for c in raw.chars() {
            if c == '"' {
                in_q = !in_q;
            }
            if c == '#' && !in_q {
                break;
            }
            line.push(c);
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (cmd, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let nums = |k: usize| -> Result<Vec<f64>> {
            let v: Vec<f64> = rest
                .split_whitespace()
                .map(|x| {
                    x.parse::<f64>()
                        .with_context(|| format!("line {n}: bad number {x:?}"))
                })
                .collect::<Result<_>>()?;
            if v.len() < k {
                bail!("line {n}: `{cmd}` needs {k} numbers");
            }
            Ok(v)
        };
        let step = match cmd {
            "wait" => Step::Wait(nums(1)?[0]),
            "tap" => {
                let v = nums(2)?;
                Step::Tap(v[0] as i32, v[1] as i32)
            }
            "long-press" => {
                let v = nums(2)?;
                Step::LongPress(v[0] as i32, v[1] as i32, v.get(2).copied().unwrap_or(1.0))
            }
            "type" => Step::Type(quoted(rest, n)?),
            "key" if !rest.trim().is_empty() => Step::Key(key_alias(rest.trim())),
            "shot" if !rest.trim().is_empty() => Step::Shot(rest.trim().to_string()),
            "expect-changed" if rest.trim().is_empty() => Step::ExpectChanged(DEFAULT_CHANGED),
            "expect-changed" => Step::ExpectChanged(nums(1)?[0] / 100.0),
            "expect-text" => Step::ExpectText(quoted(rest, n)?),
            _ => bail!("line {n}: unknown step {line:?}"),
        };
        steps.push((n, step));
    }
    Ok(steps)
}

/// Semantic media/volume key names → X keysyms understood by xdotool.
/// The fork maps these keysyms to the Android KEYCODE_MEDIA_*/KEYCODE_VOLUME_*
/// codes (src/api-impl-jni/widgets/WrapperWidget.c), so a script can drive
/// playback without knowing any coordinates. `media-play` is the hardware
/// play/pause toggle: ATL expands it per session like AOSP does, so after a
/// `media-pause` it deterministically means "play".
fn key_alias(name: &str) -> String {
    let mapped = match name {
        "media-play" | "media-play-pause" | "media-toggle" => "XF86AudioPlay",
        "media-pause" => "XF86AudioPause",
        "media-stop" => "XF86AudioStop",
        "media-next" => "XF86AudioNext",
        "media-previous" => "XF86AudioPrev",
        "media-rewind" => "XF86AudioRewind",
        "media-fast-forward" => "XF86AudioForward",
        "volume-up" => "XF86AudioRaiseVolume",
        "volume-down" => "XF86AudioLowerVolume",
        "volume-mute" => "XF86AudioMute",
        _ => return name.to_string(),
    };
    mapped.to_string()
}

fn xdotool(display: &str, args: &[&str]) -> Result<()> {
    let st = Command::new("xdotool")
        .args(args)
        .env("DISPLAY", display)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("running xdotool (install it on the runner)")?;
    if !st.success() {
        bail!("xdotool {args:?} failed: {st}");
    }
    Ok(())
}

/// Fraction of pixels that differ between two screenshots (2% colour fuzz).
pub fn changed_fraction(a: &Path, b: &Path) -> Result<f64> {
    let out = Command::new("compare")
        .args(["-metric", "AE", "-fuzz", "2%"])
        .arg(a)
        .arg(b)
        .arg("null:")
        .output()
        .context("running ImageMagick compare")?;
    // `compare` prints the differing pixel count to stderr; exit 1 = different
    let text = String::from_utf8_lossy(&out.stderr);
    let count: f64 = text
        .split_whitespace()
        .next()
        .and_then(|x| x.parse().ok())
        .with_context(|| format!("unexpected compare output {text:?}"))?;
    let id = Command::new("identify")
        .args(["-format", "%w %h"])
        .arg(a)
        .output()?;
    let dims = String::from_utf8_lossy(&id.stdout);
    let mut it = dims
        .split_whitespace()
        .filter_map(|x| x.parse::<f64>().ok());
    let (w, h) = (it.next().unwrap_or(1.0), it.next().unwrap_or(1.0));
    Ok(count / (w * h))
}

fn ocr(img: &Path) -> Result<String> {
    let out = Command::new("tesseract")
        .arg(img)
        .args(["-", "--psm", "11"])
        .stderr(Stdio::null())
        .output()
        .context("running tesseract (install it on the runner)")?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn squash(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn shoot(display: &str, dest: &Path) -> Result<()> {
    let st = Command::new("import")
        .args(["-display", display, "-window", "root"])
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !st.success() {
        bail!("import failed");
    }
    Ok(())
}

/// Run a script against a live app. `first_shot` is the settle screenshot.
pub fn run(
    steps: &[(usize, Step)],
    display: &str,
    child: &mut Child,
    out_dir: &Path,
    pkg: &str,
    first_shot: Option<PathBuf>,
) -> Interaction {
    let mut results = Vec::new();
    let mut shots: Vec<PathBuf> = first_shot.into_iter().collect();
    let mut exited = false;
    for (line, step) in steps {
        let text = format!("{step:?}");
        let r: Result<String> = (|| match step {
            Step::Wait(s) => {
                thread::sleep(Duration::from_secs_f64(*s));
                Ok(String::new())
            }
            Step::Tap(x, y) => {
                xdotool(
                    display,
                    &["mousemove", &x.to_string(), &y.to_string(), "click", "1"],
                )?;
                thread::sleep(Duration::from_millis(300));
                Ok(String::new())
            }
            Step::LongPress(x, y, s) => {
                xdotool(
                    display,
                    &[
                        "mousemove",
                        &x.to_string(),
                        &y.to_string(),
                        "mousedown",
                        "1",
                    ],
                )?;
                thread::sleep(Duration::from_secs_f64(*s));
                xdotool(display, &["mouseup", "1"])?;
                Ok(String::new())
            }
            Step::Type(t) => {
                xdotool(display, &["type", "--delay", "60", "--", t])?;
                thread::sleep(Duration::from_millis(300));
                Ok(String::new())
            }
            Step::Key(k) => {
                xdotool(display, &["key", "--", k])?;
                thread::sleep(Duration::from_millis(300));
                Ok(String::new())
            }
            Step::Shot(name) => {
                let p = out_dir.join(format!("{pkg}.{name}.png"));
                shoot(display, &p)?;
                shots.push(p.clone());
                Ok(p.display().to_string())
            }
            Step::ExpectChanged(min) => {
                if shots.len() < 2 {
                    bail!("needs two screenshots (settle + a `shot`)");
                }
                let f = changed_fraction(&shots[shots.len() - 2], &shots[shots.len() - 1])?;
                if f < *min {
                    bail!(
                        "screen unchanged ({:.4}% of pixels differ, need > {:.4}%)",
                        f * 100.0,
                        min * 100.0
                    );
                }
                Ok(format!("{:.2}% of pixels changed", f * 100.0))
            }
            Step::ExpectText(want) => {
                let last = shots.last().context("no screenshot yet (add a `shot`)")?;
                let got = ocr(last)?;
                if !squash(&got).contains(&squash(want)) {
                    bail!(
                        "text {want:?} not found; OCR saw: {:?}",
                        squash(&got).chars().take(200).collect::<String>()
                    );
                }
                Ok(format!("found {want:?}"))
            }
        })();
        let (ok, detail) = match r {
            Ok(d) => (true, d),
            Err(e) => (false, format!("{e:#}")),
        };
        results.push(StepResult {
            line: *line,
            text,
            ok,
            detail,
        });
        if matches!(child.try_wait(), Ok(Some(_))) {
            exited = true;
            break;
        }
    }
    Interaction {
        steps: results,
        exited,
    }
}

pub fn load(dir: &Path, pkg: &str) -> Result<Option<Vec<(usize, Step)>>> {
    let p = dir.join(format!("{pkg}.txt"));
    if !p.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    Ok(Some(
        parse(&text).with_context(|| format!("in {}", p.display()))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_steps() {
        let s = parse(
            "# comment\nwait 0.5\ntap 140 1050  # seven\nlong-press 10 20 2\nlong-press 1 2\n\
             type \"hello # not a comment\"\nkey Return\nshot after\nexpect-changed\nexpect-text \"15\"\nexpect-changed 2\n",
        )
        .unwrap();
        let steps: Vec<Step> = s.into_iter().map(|(_, s)| s).collect();
        assert_eq!(
            steps,
            vec![
                Step::Wait(0.5),
                Step::Tap(140, 1050),
                Step::LongPress(10, 20, 2.0),
                Step::LongPress(1, 2, 1.0),
                Step::Type("hello # not a comment".into()),
                Step::Key("Return".into()),
                Step::Shot("after".into()),
                Step::ExpectChanged(DEFAULT_CHANGED),
                Step::ExpectText("15".into()),
                Step::ExpectChanged(0.02),
            ]
        );
    }

    #[test]
    fn rejects_bad_lines() {
        for bad in [
            "tap 1",
            "type hello",
            "frobnicate",
            "wait x",
            "key",
            "expect-text 15",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn squash_normalises() {
        assert_eq!(squash("  Hello\n ATL  "), "hello atl");
    }

    #[test]
    fn media_key_aliases() {
        assert_eq!(key_alias("media-pause"), "XF86AudioPause");
        assert_eq!(key_alias("media-play"), "XF86AudioPlay");
        assert_eq!(key_alias("media-previous"), "XF86AudioPrev");
        assert_eq!(key_alias("volume-mute"), "XF86AudioMute");
        // unknown names pass through untouched (plain xdotool keysyms)
        assert_eq!(key_alias("Return"), "Return");
        // and parse() resolves the alias in the step
        let steps = parse("key media-pause").unwrap();
        assert_eq!(steps[0].1, Step::Key("XF86AudioPause".into()));
    }
}
