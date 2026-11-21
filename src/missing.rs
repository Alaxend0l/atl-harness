// SPDX-License-Identifier: AGPL-3.0-or-later
//! `atlh missing-apis`: which framework methods, fields and classes are missing,
//! aggregated over runs and ranked by how many apps hit them.
//!
//! Source: `missing_api` failures (NoSuchMethodError, NoSuchFieldError,
//! ClassNotFoundException, NoClassDefFoundError, ...) parsed from run logs.
//! ART's descriptor syntax is decoded, e.g.
//! `No virtual method setExitTransition(Landroid/transition/Transition;)V in class
//! Landroid/view/Window;` → `android.view.Window.setExitTransition(Transition)`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;
use rusqlite::{params_from_iter, Connection};
use serde::Serialize;

static METHOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"No (?P<kind>virtual|static|direct|interface|super) method (?P<name>[\w$<>]+)\((?P<args>[^)]*)\)\S* in class L(?P<cls>[^;]+);",
    )
    .unwrap()
});
static FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"No (?:(?P<kind>static|instance) )?field (?P<name>[\w$]+) of type \S+ in class L(?P<cls>[^;]+);")
        .unwrap()
});
static CLASS_DESC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:Failed resolution of: |^)L(?P<cls>[\w/$]+);").unwrap());
static CLASS_DOTTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:Didn't find class "|^)(?P<cls>[a-zA-Z_][\w$]*(?:\.[a-zA-Z_][\w$]*)+)"#)
        .unwrap()
});

const FRAMEWORK: &[&str] = &[
    "android.",
    "java.",
    "javax.",
    "dalvik.",
    "com.android.",
    "org.xmlpull.",
    "org.w3c.",
    "org.xml.",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Missing {
    /// Fully qualified class, e.g. `android.view.Window`.
    pub class: String,
    /// `setExitTransition(Transition)`, `FIELD_NAME`, or empty for a missing class.
    pub member: String,
    /// method (virtual/static/...), field, or class.
    pub what: String,
}

impl Missing {
    pub fn display(&self) -> String {
        if self.member.is_empty() {
            self.class.clone()
        } else {
            format!("{}.{}", self.class, self.member)
        }
    }

    pub fn is_framework(&self) -> bool {
        FRAMEWORK.iter().any(|p| self.class.starts_with(p))
    }
}

/// Decode a JVM method parameter descriptor list into simple type names.
fn decode_args(args: &str) -> String {
    let mut out = Vec::new();
    let b = args.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let mut dims = 0;
        while i < b.len() && b[i] == b'[' {
            dims += 1;
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let ty = match b[i] {
            b'L' => {
                let end = args[i..].find(';').map_or(b.len(), |e| i + e);
                let full = &args[i + 1..end];
                i = end;
                full.rsplit('/').next().unwrap_or(full).replace('$', ".")
            }
            c => {
                let p = match c {
                    b'Z' => "boolean",
                    b'B' => "byte",
                    b'C' => "char",
                    b'S' => "short",
                    b'I' => "int",
                    b'J' => "long",
                    b'F' => "float",
                    b'D' => "double",
                    b'V' => "void",
                    _ => "?",
                };
                p.to_string()
            }
        };
        i += 1;
        out.push(format!("{ty}{}", "[]".repeat(dims)));
    }
    out.join(", ")
}

fn class_name(desc: &str) -> String {
    desc.replace('/', ".")
}

/// Parse one `missing_api` signature (`<ExceptionType>: <message>`).
pub fn parse(signature: &str) -> Option<Missing> {
    let (ty, msg) = signature.split_once(": ")?;
    if let Some(m) = METHOD.captures(msg) {
        let name = &m["name"];
        let name = if name == "<init>" {
            "<init>".to_string()
        } else {
            name.to_string()
        };
        return Some(Missing {
            class: class_name(&m["cls"]),
            member: format!("{name}({})", decode_args(&m["args"])),
            what: format!("{} method", &m["kind"]),
        });
    }
    if let Some(m) = FIELD.captures(msg) {
        return Some(Missing {
            class: class_name(&m["cls"]),
            member: m["name"].to_string(),
            what: format!("{} field", m.name("kind").map_or("", |k| k.as_str()))
                .trim()
                .to_string(),
        });
    }
    if ty.ends_with("ClassNotFoundException") || ty.ends_with("NoClassDefFoundError") {
        let msg = msg.trim();
        let cls = CLASS_DESC
            .captures(msg)
            .map(|c| class_name(&c["cls"]))
            .or_else(|| CLASS_DOTTED.captures(msg).map(|c| c["cls"].to_string()))?;
        return Some(Missing {
            class: cls,
            member: String::new(),
            what: "class".into(),
        });
    }
    None
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub api: Missing,
    pub apps: BTreeSet<String>,
}

/// Missing APIs across the given runs (all runs if empty), most apps first.
pub fn collect(
    conn: &Connection,
    runs: &[String],
    include_app_classes: bool,
) -> Result<Vec<Entry>> {
    let mut sql = "SELECT r.pkg, f.signature FROM failures f JOIN runs r ON r.id = f.run_row \
                   WHERE f.kind = 'missing_api'"
        .to_string();
    if !runs.is_empty() {
        sql += &format!(" AND r.run_id IN ({})", vec!["?"; runs.len()].join(","));
    }
    let mut by_api: BTreeMap<Missing, BTreeSet<String>> = BTreeMap::new();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(runs), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (pkg, sig) = row?;
        if let Some(api) = parse(&sig) {
            if include_app_classes || api.is_framework() {
                by_api.entry(api).or_default().insert(pkg);
            }
        }
    }
    let mut out: Vec<Entry> = by_api
        .into_iter()
        .map(|(api, apps)| Entry { api, apps })
        .collect();
    out.sort_by(|a, b| b.apps.len().cmp(&a.apps.len()).then(a.api.cmp(&b.api)));
    Ok(out)
}

pub fn report(
    conn: &Connection,
    runs: &[String],
    include_app_classes: bool,
    json: bool,
    top: usize,
) -> Result<()> {
    let entries = collect(conn, runs, include_app_classes)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }
    let scope = if runs.is_empty() {
        "all runs".to_string()
    } else {
        runs.join(", ")
    };
    let classes: BTreeSet<&str> = entries.iter().map(|e| e.api.class.as_str()).collect();
    println!(
        "missing APIs ({scope}): {} members in {} classes",
        entries.len(),
        classes.len()
    );
    println!("apps  api                                                          kind");
    for e in entries.iter().take(top) {
        let sample: Vec<&str> = e.apps.iter().take(3).map(String::as_str).collect();
        println!(
            "{:>4}  {:<60} {:<16} {}{}",
            e.apps.len(),
            e.api.display(),
            e.api.what,
            sample.join(" "),
            if e.apps.len() > 3 { " …" } else { "" }
        );
    }
    let mut per_class: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for e in &entries {
        per_class
            .entry(&e.api.class)
            .or_default()
            .extend(e.apps.iter().map(String::as_str));
    }
    let mut cls: Vec<_> = per_class.into_iter().collect();
    cls.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    println!("\nby class (apps hitting anything missing in it):");
    for (c, apps) in cls.iter().take(15) {
        println!("{:>4}  {c}", apps.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_with_args() {
        let m = parse("java.lang.NoSuchMethodError: No virtual method setExitTransition(Landroid/transition/Transition;)V in class Landroid/view/Window; or its super classes (declaration of 'android.view.Window' appears in /x/api-impl.jar)").unwrap();
        assert_eq!(
            m.display(),
            "android.view.Window.setExitTransition(Transition)"
        );
        assert_eq!(m.what, "virtual method");
        assert!(m.is_framework());
    }

    #[test]
    fn static_method_and_primitives_arrays_inner_classes() {
        let m = parse("java.lang.NoSuchMethodError: No static method foo(I[JLandroid/view/View$OnClickListener;Z)Ljava/lang/String; in class Landroid/provider/Telephony$Sms;").unwrap();
        assert_eq!(m.class, "android.provider.Telephony$Sms");
        assert_eq!(m.member, "foo(int, long[], View.OnClickListener, boolean)");
        assert_eq!(m.what, "static method");
    }

    #[test]
    fn constructor_and_interface() {
        let m = parse("java.lang.NoSuchMethodError: No direct method <init>()V in class Landroid/media/SoundPool$Builder;").unwrap();
        assert_eq!(m.member, "<init>()");
        let m = parse("java.lang.NoSuchMethodError: No interface method addSubMenu(Ljava/lang/CharSequence;)Landroid/view/SubMenu; in class Landroid/view/Menu;").unwrap();
        assert_eq!(m.display(), "android.view.Menu.addSubMenu(CharSequence)");
    }

    #[test]
    fn fields_and_classes() {
        let m = parse("java.lang.NoSuchFieldError: No static field FLAG_X of type I in class Landroid/view/WindowManager$LayoutParams;").unwrap();
        assert_eq!(
            (m.display().as_str(), m.what.as_str()),
            (
                "android.view.WindowManager$LayoutParams.FLAG_X",
                "static field"
            )
        );
        let m =
            parse("java.lang.ClassNotFoundException: android.text.InputFilter$AllCaps").unwrap();
        assert_eq!(
            (m.class.as_str(), m.what.as_str()),
            ("android.text.InputFilter$AllCaps", "class")
        );
        let m = parse("java.lang.NoClassDefFoundError: Failed resolution of: Landroid/appwidget/AppWidgetHost;").unwrap();
        assert_eq!(m.class, "android.appwidget.AppWidgetHost");
        let m = parse("java.lang.ClassNotFoundException: Didn't find class \"android.webkit.VideoView\" on path: x").unwrap();
        assert_eq!(m.class, "android.webkit.VideoView");
    }

    #[test]
    fn app_classes_are_not_framework() {
        let m = parse("java.lang.NoSuchMethodError: No virtual method setThumbOffset(I)V in class Lcom/cax/pmk/widget/Slider;").unwrap();
        assert!(!m.is_framework());
        assert!(parse("java.lang.UnsatisfiedLinkError: dlopen failed").is_none());
    }

    #[test]
    fn collect_counts_distinct_apps() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE runs(id INTEGER PRIMARY KEY, run_id TEXT, pkg TEXT);
             CREATE TABLE failures(run_row INTEGER, kind TEXT, signature TEXT);
             INSERT INTO runs VALUES (1,'a','app1'),(2,'a','app2'),(3,'b','app1');
             INSERT INTO failures VALUES
               (1,'missing_api','java.lang.ClassNotFoundException: android.net.VpnService'),
               (2,'missing_api','java.lang.ClassNotFoundException: android.net.VpnService'),
               (3,'missing_api','java.lang.ClassNotFoundException: android.net.VpnService'),
               (3,'missing_api','java.lang.NoSuchMethodError: No virtual method x()V in class Lcom/app/X;'),
               (1,'exception','java.lang.NullPointerException @ ?');",
        )
        .unwrap();
        let all = collect(&conn, &[], false).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].apps.len(), 2);
        assert_eq!(collect(&conn, &["b".into()], true).unwrap().len(), 2);
    }
}
