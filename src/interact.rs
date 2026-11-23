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
//!   find "NAME" [class=…] [nth=N]
//!                         resolve a widget by its text / content-description
//!                         (case-insensitive; an exact match beats substring
//!                         ones, `class=` filters by widget class, 1-based
//!                         `nth=` picks among matches) from the app's live
//!                         view tree; the match becomes `$el`. Needs the
//!                         fork's ATL_ELEMENT_DUMP dump, which the harness
//!                         enables automatically when a script uses these steps
//!   wait-find "NAME" …    same, retrying for up to `timeout SECS` (default 10 s)
//!   tap $el               click the centre of the last found element
//!   long-press $el [SECS] hold the centre of the last found element
//! A failed `find`/`wait-find` records the widget list the app exposes into the
//! run log, so a missing element is a real signal, not a silent mis-tap.
//! After every step the app must still be running; if it exits, the script stops
//! and the run is classified from its exit like any crash.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

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
    /// resolve a widget from the app's element dump; `timeout_s` > 0 retries
    Find(FindSpec),
    /// click/hold the centre of the element found by the last Find
    TapEl,
    LongPressEl(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FindSpec {
    pub name: String,
    pub class: Option<String>,
    /// 1-based match index among matching widgets
    pub nth: usize,
    pub timeout_s: f64,
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
            "tap" if rest.trim() == "$el" => Step::TapEl,
            "tap" => {
                let v = nums(2)?;
                Step::Tap(v[0] as i32, v[1] as i32)
            }
            "long-press" if rest.trim().starts_with("$el") => {
                let r = rest.trim()[3..].trim();
                let secs = match r {
                    "" => 1.0,
                    x => x
                        .parse::<f64>()
                        .with_context(|| format!("line {n}: bad hold seconds {x:?}"))?,
                };
                Step::LongPressEl(secs)
            }
            "long-press" => {
                let v = nums(2)?;
                Step::LongPress(v[0] as i32, v[1] as i32, v.get(2).copied().unwrap_or(1.0))
            }
            "type" => Step::Type(quoted(rest, n)?),
            "key" if !rest.trim().is_empty() => Step::Key(key_alias(rest.trim())),
            "shot" if !rest.trim().is_empty() => Step::Shot(rest.trim().to_string()),
            "find" => Step::Find(parse_find(rest, n, 2.0)?),
            "wait-find" => Step::Find(parse_find(rest, n, 10.0)?),
            "expect-changed" if rest.trim().is_empty() => Step::ExpectChanged(DEFAULT_CHANGED),
            "expect-changed" => Step::ExpectChanged(nums(1)?[0] / 100.0),
            "expect-text" => Step::ExpectText(quoted(rest, n)?),
            _ => bail!("line {n}: unknown step {line:?}"),
        };
        steps.push((n, step));
    }
    Ok(steps)
}

/// `find`/`wait-find` tail: `"NAME" [class=CLASS] [nth=N] [timeout SECS]`.
fn parse_find(rest: &str, line: usize, default_timeout: f64) -> Result<FindSpec> {
    let r = rest.trim();
    if !r.starts_with('"') {
        bail!("line {line}: `find` needs a \"quoted\" element name");
    }
    let end = r[1..]
        .find('"')
        .map(|i| i + 1)
        .with_context(|| format!("line {line}: unterminated quote"))?;
    let mut spec = FindSpec {
        name: quoted(&r[..=end], line)?,
        class: None,
        nth: 1,
        timeout_s: default_timeout,
    };
    for tok in r[end + 1..].split_whitespace() {
        if let Some(v) = tok.strip_prefix("class=") {
            spec.class = Some(v.to_string());
        } else if let Some(v) = tok.strip_prefix("nth=") {
            spec.nth = v
                .parse::<usize>()
                .with_context(|| format!("line {line}: bad nth={v:?}"))?;
            if spec.nth < 1 {
                bail!("line {line}: nth= is 1-based");
            }
        } else if let Some(v) = tok.strip_prefix("timeout=") {
            spec.timeout_s = v
                .parse::<f64>()
                .with_context(|| format!("line {line}: bad timeout={v:?}"))?;
        } else if let Ok(v) = tok.parse::<f64>() {
            spec.timeout_s = v; // bare number, as in the planning brief
        } else {
            bail!("line {line}: unknown find option {tok:?}");
        }
    }
    Ok(spec)
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

fn xdotool_out(display: &str, args: &[&str]) -> Result<String> {
    let out = Command::new("xdotool")
        .args(args)
        .env("DISPLAY", display)
        .stderr(Stdio::null())
        .output()
        .context("running xdotool (install it on the runner)")?;
    if !out.status.success() {
        bail!("xdotool {args:?} failed: {}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `xdotool getwindowgeometry --shell` output → window position.
/// xdotool prints `X=`/`Y=`; accept `POSITION_X=` too (newer builds).
fn parse_shell_geometry(s: &str) -> Option<(i32, i32)> {
    let mut x = None;
    let mut y = None;
    for line in s.lines() {
        let line = line.trim();
        if let Some(v) = line
            .strip_prefix("X=")
            .or_else(|| line.strip_prefix("POSITION_X="))
        {
            x = v.parse().ok();
        } else if let Some(v) = line
            .strip_prefix("Y=")
            .or_else(|| line.strip_prefix("POSITION_Y="))
        {
            y = v.parse().ok();
        }
    }
    Some((x?, y?))
}

/// Origin of the app window in root-window pixels: element dumps give
/// window-local bounds, taps and screenshots are root-window absolute.
/// (Not the blank-scoring crop: that is a scoring crop, not an origin.)
fn window_origin(display: &str) -> Result<(i32, i32)> {
    let ids = xdotool_out(display, &["search", "--class", "android-translation-layer"])
        .or_else(|_| xdotool_out(display, &["search", "--name", "."]))?;
    let id = ids
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .context("no ATL window on this display (element taps need one)")?;
    let g = xdotool_out(display, &["getwindowgeometry", "--shell", id])?;
    parse_shell_geometry(&g).context("cannot read ATL window position")
}

/// Screen size of the display: `xdotool getdisplaygeometry` prints "W H".
fn display_size(display: &str) -> Result<(u32, u32)> {
    let out = xdotool_out(display, &["getdisplaygeometry"])?;
    let mut it = out.split_whitespace();
    let w = it
        .next()
        .context("empty getdisplaygeometry")?
        .parse()
        .context("bad getdisplaygeometry width")?;
    let h = it
        .next()
        .context("empty getdisplaygeometry")?
        .parse()
        .context("bad getdisplaygeometry height")?;
    Ok((w, h))
}

/// One node of the fork's ATL_ELEMENT_DUMP views.json (android.atl.ATLElementDump).
#[derive(Debug, Deserialize)]
struct DumpNode {
    /// class name
    c: String,
    /// window-local bounds
    l: i32,
    t: i32,
    r: i32,
    b: i32,
    #[serde(default = "yes")]
    en: bool,
    #[serde(default)]
    #[allow(dead_code)] // kept for future find options (e.g. clickable-only)
    clk: bool,
    #[serde(default)]
    txt: Option<String>,
    #[serde(default)]
    desc: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct Dump {
    #[serde(default)]
    nodes: Vec<DumpNode>,
}

/// A resolved element: centre in root-window pixels plus a human label.
struct Element {
    cx: i32,
    cy: i32,
    label: String,
}

fn read_dump(dir: &Path) -> Option<Vec<DumpNode>> {
    let f = std::fs::File::open(dir.join("views.json")).ok()?;
    serde_json::from_reader::<_, Dump>(f).ok().map(|d| d.nodes)
}

fn node_matches(n: &DumpNode, spec: &FindSpec) -> bool {
    if !n.en {
        return false;
    }
    if let Some(cls) = &spec.class {
        if !n.c.contains(cls.as_str()) {
            return false;
        }
    }
    let needle = squash(&spec.name);
    if needle.is_empty() {
        return false;
    }
    n.txt
        .as_deref()
        .map(squash)
        .into_iter()
        .chain(n.desc.as_deref().map(squash))
        .any(|hay| !hay.is_empty() && (hay == needle || hay.contains(&needle)))
}

fn node_label(n: &DumpNode) -> String {
    let name = n
        .txt
        .as_deref()
        .or(n.desc.as_deref())
        .map(|s| s.replace('\n', " ").chars().take(40).collect::<String>());
    match name {
        Some(s) => format!("{:?} ({})", s, n.c),
        None => format!("({})", n.c),
    }
}

fn pick_match(
    nodes: &[DumpNode],
    spec: &FindSpec,
    on_screen: impl Fn(&DumpNode) -> bool,
) -> Option<(usize, usize)> {
    // Exact text/description beats substring matches: find "Tracks" should
    // hit the tab labelled "Tracks", not a "9 Tracks" subheader.
    let needle = squash(&spec.name);
    let hits: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| node_matches(n, spec) && on_screen(n))
        .map(|(i, _)| i)
        .collect();
    let total = hits.len();
    let exact: Vec<usize> = hits
        .iter()
        .copied()
        .filter(|i| {
            let n = &nodes[*i];
            n.txt.as_deref().map(squash).as_deref() == Some(needle.as_str())
                || n.desc.as_deref().map(squash).as_deref() == Some(needle.as_str())
        })
        .collect();
    let pool: &[_] = if exact.is_empty() { &hits } else { &exact };
    pool.get(spec.nth - 1).copied().map(|i| (i, total))
}

fn candidate_list(nodes: &[DumpNode], cap: usize) -> String {
    let named: Vec<String> = nodes
        .iter()
        .filter(|n| n.txt.is_some() || n.desc.is_some())
        .map(node_label)
        .take(cap)
        .collect();
    if named.is_empty() {
        format!(
            "{} nodes, none with text or content-description",
            nodes.len()
        )
    } else {
        format!(
            "{} of {} nodes: {}",
            named.len(),
            nodes.len(),
            named.join(", ")
        )
    }
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
/// `elem_dir` is the directory the fork was told to dump its view tree into
/// (`ATL_ELEMENT_DUMP`); required by scripts that use `find`/`tap $el`.
pub fn run(
    steps: &[(usize, Step)],
    display: &str,
    child: &mut Child,
    out_dir: &Path,
    pkg: &str,
    first_shot: Option<PathBuf>,
    elem_dir: Option<&Path>,
) -> Interaction {
    let mut results = Vec::new();
    let mut shots: Vec<PathBuf> = first_shot.into_iter().collect();
    let mut exited = false;
    let mut last_el: Option<Element> = None;
    let mut origin: Option<(i32, i32)> = None;
    let mut screen: Option<(u32, u32)> = None;
    for (line, step) in steps {
        let text = format!("{step:?}");
        let r: Result<String> = (|| match step {
            Step::Wait(s) => {
                thread::sleep(Duration::from_secs_f64(*s));
                Ok(String::new())
            }
            Step::Find(spec) => {
                let dir = elem_dir
                    .context("script uses find/tap $el but no element dump dir was set up")?;
                let _ = fs::write(dir.join("request"), b"");
                let deadline = Instant::now() + Duration::from_secs_f64(spec.timeout_s.max(0.5));
                loop {
                    if let Some(nodes) = read_dump(dir) {
                        let (ox, oy) = match origin {
                            Some(o) => o,
                            None => {
                                let o = window_origin(display)?;
                                origin = Some(o);
                                o
                            }
                        };
                        // off-window nodes (ViewPager neighbours, scrolled
                        // list items) can never receive the tap: skip them
                        let (sw, sh) = match screen {
                            Some(s) => s,
                            None => {
                                let s = display_size(display)?;
                                screen = Some(s);
                                s
                            }
                        };
                        let on_screen = |n: &DumpNode| {
                            let cx = (n.l + n.r) / 2 + ox;
                            let cy = (n.t + n.b) / 2 + oy;
                            cx >= 0 && cy >= 0 && cx < sw as i32 && cy < sh as i32
                        };
                        if let Some((i, m)) = pick_match(&nodes, spec, on_screen) {
                            let n = &nodes[i];
                            let el = Element {
                                cx: (n.l + n.r) / 2 + ox,
                                cy: (n.t + n.b) / 2 + oy,
                                label: node_label(n),
                            };
                            let det = format!(
                                "$el = {} at ({},{}) {}x{}, match {} of {}, win ({ox},{oy})",
                                el.label,
                                el.cx,
                                el.cy,
                                n.r - n.l,
                                n.b - n.t,
                                spec.nth,
                                m
                            );
                            // keep the exact tree the match came from, for the run record
                            let _ = fs::copy(dir.join("views.json"), out_dir.join("elements.json"));
                            last_el = Some(el);
                            return Ok(det);
                        }
                    }
                    if Instant::now() >= deadline {
                        match read_dump(dir) {
                            Some(nodes) => bail!(
                                "no element {:?} (class={:?} nth={}) within {:.1}s; app exposes {}",
                                spec.name,
                                spec.class,
                                spec.nth,
                                spec.timeout_s,
                                candidate_list(&nodes, 40)
                            ),
                            None => bail!(
                                "no element dump in {} (does the installed ATL build support ATL_ELEMENT_DUMP?)",
                                dir.display()
                            ),
                        }
                    }
                    thread::sleep(Duration::from_millis(250));
                }
            }
            Step::TapEl => {
                let el = last_el
                    .as_ref()
                    .context("no element found yet (add a `find` before `tap $el`)")?;
                xdotool(
                    display,
                    &[
                        "mousemove",
                        &el.cx.to_string(),
                        &el.cy.to_string(),
                        "click",
                        "1",
                    ],
                )?;
                thread::sleep(Duration::from_millis(300));
                Ok(el.label.clone())
            }
            Step::LongPressEl(s) => {
                let el = last_el
                    .as_ref()
                    .context("no element found yet (add a `find` before `long-press $el`)")?;
                xdotool(
                    display,
                    &[
                        "mousemove",
                        &el.cx.to_string(),
                        &el.cy.to_string(),
                        "mousedown",
                        "1",
                    ],
                )?;
                thread::sleep(Duration::from_secs_f64(*s));
                xdotool(display, &["mouseup", "1"])?;
                Ok(el.label.clone())
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

/// True if a script needs the fork's element dump (`find`/`tap $el`).
pub fn uses_elements(steps: &[(usize, Step)]) -> bool {
    steps
        .iter()
        .any(|(_, s)| matches!(s, Step::Find(_) | Step::TapEl | Step::LongPressEl(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_steps() {
        let s = parse(
            "# comment\nwait 0.5\ntap 140 1050  # seven\nlong-press 10 20 2\nlong-press 1 2\n\
             type \"hello # not a comment\"\nkey Return\nshot after\nexpect-changed\nexpect-text \"15\"\nexpect-changed 2\n\
             find \"Play\" class=Button nth=2\nwait-find \"Now playing\" 5\nwait-find \"X\" timeout=1.5 class=T_ab\n\
             tap $el\nlong-press $el\nlong-press $el 2.5\n",
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
                Step::Find(FindSpec {
                    name: "Play".into(),
                    class: Some("Button".into()),
                    nth: 2,
                    timeout_s: 2.0,
                }),
                Step::Find(FindSpec {
                    name: "Now playing".into(),
                    class: None,
                    nth: 1,
                    timeout_s: 5.0,
                }),
                Step::Find(FindSpec {
                    name: "X".into(),
                    class: Some("T_ab".into()),
                    nth: 1,
                    timeout_s: 1.5,
                }),
                Step::TapEl,
                Step::LongPressEl(1.0),
                Step::LongPressEl(2.5),
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
            "find Play",
            "find \"Play\" frobnicate",
            "find \"Play\" nth=0",
            "tap $el x",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    const FIXTURE: &str = r#"{"v":1,"nodes":[
        {"c":"android.widget.FrameLayout","l":0,"t":0,"r":1080,"b":1920},
        {"c":"android.widget.TextView","l":10,"t":100,"r":110,"b":140,"txt":"Play"},
        {"c":"android.widget.ImageButton","l":200,"t":300,"r":280,"b":380,"desc":"Pause","clk":true},
        {"c":"android.widget.TextView","l":10,"t":500,"r":110,"b":540,"txt":"play","en":false},
        {"c":"com.example.NavItem","l":0,"t":1800,"r":270,"b":1920,"txt":"Now  playing"}
    ]}"#;

    fn fixture_nodes() -> Vec<DumpNode> {
        serde_json::from_str::<Dump>(FIXTURE).unwrap().nodes
    }

    fn spec(name: &str, class: Option<&str>, nth: usize) -> FindSpec {
        FindSpec {
            name: name.into(),
            class: class.map(str::to_string),
            nth,
            timeout_s: 0.0,
        }
    }

    #[test]
    fn finds_by_text_or_desc() {
        let nodes = fixture_nodes();
        // plain text, case-insensitive; substring hits "Now playing" too (2 hits),
        // but the exact "Play" node wins first
        assert_eq!(
            pick_match(&nodes, &spec("play", None, 1), |_| true),
            Some((1, 2))
        );
        // nth indexes the exact-match pool when one exists (only "Play" is exact)
        assert_eq!(pick_match(&nodes, &spec("play", None, 2), |_| true), None);
        // content-description counts too
        assert_eq!(
            pick_match(&nodes, &spec("pause", None, 1), |_| true),
            Some((2, 1))
        );
        // nth beyond every hit -> None (also proves the disabled "play" node never counted)
        assert_eq!(
            pick_match(&nodes, &spec("play", Some("TextView"), 2), |_| true),
            None
        );
        // substring + whitespace-squashed match (exact once squashed)
        assert_eq!(
            pick_match(&nodes, &spec("now playing", None, 1), |_| true),
            Some((4, 1))
        );
        // class filter
        assert_eq!(
            pick_match(&nodes, &spec("play", Some("ImageButton"), 1), |_| true),
            None
        );
        assert_eq!(
            pick_match(&nodes, &spec("play", Some("TextView"), 1), |_| true),
            Some((1, 1))
        );
        // structural nodes (no text/desc) do not match a name
        assert_eq!(pick_match(&nodes, &spec("frame", None, 1), |_| true), None);
    }

    #[test]
    fn candidate_list_names_elements() {
        let list = candidate_list(&fixture_nodes(), 40);
        assert!(
            list.contains("\"Play\" (android.widget.TextView)"),
            "{list}"
        );
        assert!(
            list.contains("\"Pause\" (android.widget.ImageButton)"),
            "{list}"
        );
        assert!(!list.contains("FrameLayout"), "{list}");
    }

    #[test]
    fn geometry_shell_parsed() {
        assert_eq!(
            parse_shell_geometry("WINDOW=6291458\nX=0\nY=0\nWIDTH=1080\nHEIGHT=1920\n"),
            Some((0, 0))
        );
        assert_eq!(
            parse_shell_geometry(
                "WINDOW=6291458\nPOSITION_X=120\nPOSITION_Y=64\nWIDTH=1080\nHEIGHT=1920\n"
            ),
            Some((120, 64))
        );
        assert_eq!(parse_shell_geometry("WINDOW=1\nWIDTH=2\n"), None);
    }

    #[test]
    fn element_scripts_flag_the_dump() {
        assert!(uses_elements(&parse("find \"Play\"\ntap $el").unwrap()));
        assert!(!uses_elements(&parse("tap 10 20\nkey media-play").unwrap()));
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
