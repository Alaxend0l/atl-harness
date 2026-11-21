//! Turn an ATL run log into deduplicated failure signatures.

use std::collections::HashSet;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

static EXC_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|[^\w$.])(?P<cause>Caused by:\s*)?(?P<ty>(?:[a-zA-Z_][\w$]*\.)+[A-Z][\w$]*(?:Exception|Error|Throwable))(?::\s*(?P<msg>.*?))?\s*$",
    )
    .unwrap()
});
static FRAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)at\s+(?P<frame>[\w$.<>-]+)\s*\(").unwrap());
static NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0x[0-9a-fA-F]+|\b\d+\b").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static JNI_PENDING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"JNI DETECTED ERROR IN APPLICATION: (JNI \w+ called with pending exception)")
        .unwrap()
});
static JNI_OTHER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"JNI DETECTED ERROR IN APPLICATION: (.+)").unwrap());
/// Chromium-style fatal log (Flutter engine, WebView): `[FATAL:path/file.cc(25)] message`.
static FATAL_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[FATAL:([^\]()]+)(?:\(\d+\))?\]\s*(.*)").unwrap());
static SWALLOWED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"W/ATL: swallowed exception in (\w+)").unwrap());
static ABORT_MSG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Abort message: '(.+?)'?\s*$").unwrap());

const MISSING_API: &[&str] = &[
    "java.lang.NoSuchMethodError",
    "java.lang.NoSuchFieldError",
    "java.lang.ClassNotFoundException",
    "java.lang.NoClassDefFoundError",
    "java.lang.UnsatisfiedLinkError",
    "java.lang.AbstractMethodError",
];
const FRAMEWORK_PREFIXES: &[&str] = &["android.", "com.android."];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    MissingApi,
    Exception,
    Stub,
    Signal,
    /// ATL logged and cleared an exception from a callback (fork: doc/Exceptions.md);
    /// Android would have ended the app.
    Swallowed,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::MissingApi => "missing_api",
            Kind::Exception => "exception",
            Kind::Stub => "stub",
            Kind::Signal => "signal",
            Kind::Swallowed => "swallowed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: Kind,
    pub signature: String,
    pub detail: String,
}

pub fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| Regex::new(p).with_context(|| format!("bad stub pattern {p:?}")))
        .collect()
}

pub fn normalize(s: &str) -> String {
    let s = NUMBERS.replace_all(s, "N");
    let s = SPACES.replace_all(&s, " ");
    s.trim().chars().take(300).collect()
}

struct Block {
    ty: String,
    msg: Option<String>,
    frames: Vec<String>,
}

fn signature(chain: &[Block]) -> Failure {
    let root = chain.last().expect("chains are never empty");
    let frames = if root.frames.is_empty() {
        &chain[0].frames
    } else {
        &root.frames
    };
    let detail = chain
        .iter()
        .map(|b| format!("{}: {}", b.ty, b.msg.as_deref().unwrap_or("")))
        .chain(frames.iter().take(10).map(|f| format!("  at {f}")))
        .collect::<Vec<_>>()
        .join("\n");
    let msg = root.msg.as_deref().unwrap_or("");
    if MISSING_API.contains(&root.ty.as_str()) && !msg.is_empty() {
        return Failure {
            kind: Kind::MissingApi,
            signature: format!("{}: {}", root.ty, normalize(msg)),
            detail,
        };
    }
    let top = frames
        .iter()
        .find(|f| FRAMEWORK_PREFIXES.iter().any(|p| f.starts_with(p)))
        .or(frames.first())
        .map_or("?", String::as_str);
    Failure {
        kind: Kind::Exception,
        signature: format!("{} @ {top}", root.ty),
        detail,
    }
}

/// Why a process aborted, if the log says: the JNI checker's complaint (an ATL
/// native-glue bug when an exception is pending), a `[FATAL:...]` engine
/// message, or a bionic abort message. Used to split `signal: SIGABRT`.
pub fn abort_reason(text: &str) -> Option<String> {
    let first = |re: &Regex, f: &dyn Fn(&regex::Captures) -> String| {
        text.lines().find_map(|l| re.captures(l).map(|c| f(&c)))
    };
    first(&JNI_PENDING, &|c| c[1].to_string())
        .or_else(|| first(&JNI_OTHER, &|c| format!("JNI {}", c[1].trim())))
        .or_else(|| {
            first(&FATAL_TAG, &|c| {
                format!("[FATAL:{}] {}", &c[1], c[2].trim())
            })
        })
        .or_else(|| first(&ABORT_MSG, &|c| c[1].to_string()))
        .map(|r| normalize(&r).chars().take(160).collect())
}

/// The runtime has started aborting (it may still be dumping threads).
pub fn is_aborting(text: &str) -> bool {
    text.lines().any(|l| {
        l.contains("Runtime aborting")
            || l.starts_with("Fatal signal ")
            || l.contains("JNI DETECTED ERROR IN APPLICATION")
    })
}

pub fn parse(text: &str, stubs: &[Regex]) -> Vec<Failure> {
    let mut chains: Vec<Vec<Block>> = Vec::new();
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(m) = SWALLOWED.captures(line) {
            out.push(Failure {
                kind: Kind::Swallowed,
                signature: format!("swallowed in {}", &m[1]),
                detail: line.trim().to_string(),
            });
            continue;
        }
        if stubs.iter().any(|p| p.is_match(line)) {
            out.push(Failure {
                kind: Kind::Stub,
                signature: format!("stub: {}", normalize(line)),
                detail: line.trim().to_string(),
            });
        }
        if let Some(m) = FRAME.captures(line) {
            if let Some(block) = chains.last_mut().and_then(|c| c.last_mut()) {
                block.frames.push(m["frame"].to_string());
            }
            continue;
        }
        if let Some(m) = EXC_LINE.captures(line) {
            let block = Block {
                ty: m["ty"].to_string(),
                msg: m.name("msg").map(|x| x.as_str().to_string()),
                frames: Vec::new(),
            };
            match chains.last_mut() {
                Some(chain) if m.name("cause").is_some() => chain.push(block),
                _ => chains.push(vec![block]),
            }
        }
    }
    out.extend(chains.iter().map(|c| signature(c)));
    let mut seen = HashSet::new();
    out.retain(|f| seen.insert(f.signature.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
some ATL startup noise
STUB: android.app.NotificationManager.createNotificationChannel
java.lang.RuntimeException: Unable to start activity
\tat android.app.ActivityThread.performLaunchActivity(ActivityThread.java:120)
\tat com.example.Main.main(Main.java:5)
Caused by: java.lang.NoSuchMethodError: No virtual method setElevation(F)V in class Landroid/view/View;
\tat com.example.ui.Card.<init>(Card.java:42)
\t... 3 more
W/System.err: java.lang.IllegalStateException: boom 0x7f3a12
\tat com.example.Foo.bar(Foo.java:1)
\tat android.widget.TextView.setText(TextView.java:99)
java.lang.IllegalStateException: boom 0x11
\tat com.example.Foo.bar(Foo.java:1)
\tat android.widget.TextView.setText(TextView.java:99)
";

    fn sigs() -> Vec<Failure> {
        parse(LOG, &compile_patterns(&[r"(?i)\bstub\b".into()]).unwrap())
    }

    fn has(sig: &str) -> bool {
        sigs().iter().any(|f| f.signature == sig)
    }

    #[test]
    fn stub_line() {
        assert!(has(
            "stub: STUB: android.app.NotificationManager.createNotificationChannel"
        ));
    }

    #[test]
    fn root_cause_missing_api() {
        assert!(has("java.lang.NoSuchMethodError: No virtual method setElevation(F)V in class Landroid/view/View;"));
        assert!(!has(
            "java.lang.RuntimeException @ android.app.ActivityThread.performLaunchActivity"
        ));
    }

    #[test]
    fn framework_frame_and_dedup() {
        assert!(has(
            "java.lang.IllegalStateException @ android.widget.TextView.setText"
        ));
        assert_eq!(sigs().len(), 3);
    }

    #[test]
    fn abort_reasons() {
        let jni = "A/android-translation-layer(33994): java_vm_ext.cc:563] JNI DETECTED ERROR IN APPLICATION: JNI GetMethodID called with pending exception java.lang.NoSuchMethodError: No virtual method x\n";
        assert_eq!(
            abort_reason(jni).as_deref(),
            Some("JNI GetMethodID called with pending exception")
        );
        let other = "x] JNI DETECTED ERROR IN APPLICATION: use of deleted local reference 0x71\n";
        assert_eq!(
            abort_reason(other).as_deref(),
            Some("JNI use of deleted local reference N")
        );
        let flutter = " A/flutter (33430): [FATAL:flutter/shell/platform/android/library_loader.cc(25)] Check failed: result. \n";
        assert_eq!(
            abort_reason(flutter).as_deref(),
            Some("[FATAL:flutter/shell/platform/android/library_loader.cc] Check failed: result.")
        );
        assert_eq!(
            abort_reason("Abort message: 'bad thing 12'\n").as_deref(),
            Some("bad thing N")
        );
        assert_eq!(abort_reason("nothing here\n"), None);
    }

    #[test]
    fn swallowed_exceptions() {
        let log = "W/ATL: swallowed exception in wrapper_widget_draw (logged, cleared, continuing; Android would end the app):\njava.lang.NullPointerException: boom\n\tat android.view.View.draw(View.java:1)\n";
        let f = parse(log, &[]);
        assert!(f.iter().any(
            |f| f.kind == Kind::Swallowed && f.signature == "swallowed in wrapper_widget_draw"
        ));
        assert!(f
            .iter()
            .any(|f| f.signature == "java.lang.NullPointerException @ android.view.View.draw"));
    }

    #[test]
    fn aborting_detection() {
        assert!(is_aborting(
            "x\nA/art(1): JNI DETECTED ERROR IN APPLICATION: y\n"
        ));
        assert!(is_aborting(" Runtime aborting...\n"));
        assert!(is_aborting("Fatal signal 6 (SIGABRT), code -6\n"));
        assert!(!is_aborting("W/x: everything is fine\n"));
    }

    #[test]
    fn normalize_strips_numbers() {
        assert_eq!(normalize("boom 0x7f3a12  at   42"), "boom N at N");
    }
}
