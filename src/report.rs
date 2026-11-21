// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ranking, regression gate, and agent work items.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::config::Config;
use crate::runner::Outcome;

const RANK_SQL: &str = "
WITH per_run AS (SELECT run_row, COUNT(*) AS n FROM failures GROUP BY run_row)
SELECT f.kind, f.signature,
       COUNT(*) AS apps,
       SUM(r.outcome != 'ALIVE') AS broken,
       SUM(r.outcome != 'ALIVE' AND p.n = 1) AS sole,
       GROUP_CONCAT(r.pkg, ' ') AS pkgs
FROM failures f
JOIN runs r ON r.id = f.run_row
JOIN per_run p ON p.run_row = f.run_row
WHERE r.run_id = ?1
GROUP BY f.kind, f.signature
ORDER BY sole DESC, broken DESC, apps DESC
LIMIT ?2
";

#[derive(Debug)]
pub struct Ranked {
    pub kind: String,
    pub signature: String,
    pub apps: i64,
    pub broken: i64,
    /// Broken apps where this is the only signature seen.
    pub sole: i64,
    pub pkgs: String,
}

pub fn ranked(conn: &Connection, run_id: &str, top: usize) -> Result<Vec<Ranked>> {
    let mut stmt = conn.prepare(RANK_SQL)?;
    let rows = stmt
        .query_map(params![run_id, top as i64], |r| {
            Ok(Ranked {
                kind: r.get(0)?,
                signature: r.get(1)?,
                apps: r.get(2)?,
                broken: r.get(3)?,
                sole: r.get(4)?,
                pkgs: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn rank(conn: &Connection, run_id: &str, top: usize) -> Result<()> {
    println!("run {run_id}");
    let mut stmt = conn.prepare(
        "SELECT outcome, COUNT(*) AS n FROM runs WHERE run_id=?1 GROUP BY outcome ORDER BY n DESC",
    )?;
    for row in stmt.query_map([run_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })? {
        let (outcome, n) = row?;
        println!("  {outcome:<14}{n}");
    }
    let blind: i64 = conn.query_row(
        "SELECT COUNT(*) FROM runs r WHERE run_id=?1 AND outcome!='ALIVE'
         AND NOT EXISTS (SELECT 1 FROM failures f WHERE f.run_row=r.id)",
        [run_id],
        |r| r.get(0),
    )?;
    println!("  broken with no signature (classifier blind spots): {blind}\n");
    println!(
        "{:>3} {:>4} {:>4} {:>4}  signature",
        "#", "sole", "brk", "apps"
    );
    for (i, r) in ranked(conn, run_id, top)?.iter().enumerate() {
        println!(
            "{:>3} {:>4} {:>4} {:>4}  [{}] {}",
            i + 1,
            r.sole,
            r.broken,
            r.apps,
            r.kind,
            r.signature
        );
    }
    Ok(())
}

/// Outcome plus the interaction script's result (None: no script, or not run).
type Result2 = (Outcome, Option<bool>);

fn outcomes(conn: &Connection, run_id: &str) -> Result<HashMap<String, Result2>> {
    let mut stmt = conn.prepare("SELECT pkg, outcome, interact_ok FROM runs WHERE run_id=?1")?;
    let rows = stmt.query_map([run_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<bool>>(2)?,
        ))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (pkg, outcome, interact) = row?;
        map.insert(pkg, (outcome.parse()?, interact));
    }
    Ok(map)
}

fn label((o, i): Result2) -> String {
    match i {
        Some(true) => format!("{}+ok", o.as_str()),
        Some(false) => format!("{}+FAIL", o.as_str()),
        None => o.as_str().to_string(),
    }
}

/// -1 worse, 0 same, 1 better. Outcome first; with the same outcome, an
/// interaction script that passed before and fails now is a regression
/// (and the reverse an improvement). Apps without a script in either run
/// compare by outcome only.
pub fn verdict(x: Result2, y: Result2) -> i8 {
    use std::cmp::Ordering::*;
    match y.0.score().cmp(&x.0.score()) {
        Greater => 1,
        Less => -1,
        Equal => match (x.1, y.1) {
            (Some(true), Some(false)) => -1,
            (Some(false), Some(true)) => 1,
            _ => 0,
        },
    }
}

/// Regression gate. Returns the number of regressed apps.
pub fn compare(conn: &Connection, base: &str, new: &str) -> Result<usize> {
    let (a, b) = (outcomes(conn, base)?, outcomes(conn, new)?);
    let mut common: Vec<&String> = a.keys().filter(|k| b.contains_key(*k)).collect();
    common.sort();
    let (mut better, mut worse) = (Vec::new(), Vec::new());
    for pkg in common.iter().copied() {
        let (x, y) = (a[pkg], b[pkg]);
        match verdict(x, y) {
            1 => better.push((pkg, x, y)),
            -1 => worse.push((pkg, x, y)),
            _ => {}
        }
    }
    println!(
        "{} apps in both runs; {} improved, {} regressed",
        common.len(),
        better.len(),
        worse.len()
    );
    for (p, x, y) in &better {
        println!("  + {p}: {} -> {}", label(*x), label(*y));
    }
    for (p, x, y) in &worse {
        println!("  - {p}: {} -> {}", label(*x), label(*y));
    }
    if !worse.is_empty() {
        println!(
            "GATE FAILED (rerun regressed apps with `run --force --pkg ...` to rule out flakiness)"
        );
    }
    Ok(worse.len())
}

const PROMPT: &str = r#"# Work item: {signature}

You are working in a git worktree of a hard fork of Android Translation Layer
(ATL), which runs Android apps on Linux by reimplementing the Android
framework on top of GTK. Fix the failure below so the affected apps progress
further.

## Failure
Kind: {kind}
Affected apps ({apps}, broken {broken}, sole blocker in {sole}): {pkgs}

Sample trace:
```
{detail}
```

## Reference
- ATL source: this worktree
- AOSP framework source (reference behaviour, Apache-2.0): {aosp}

## Rules
- Implement real behaviour based on AOSP, adapted to ATL's architecture.
- Do NOT silence the failure: no catch-all try/catch, no returning null/defaults
  from methods that should do work. If a faithful implementation is not
  feasible, add a clearly logged stub and explain why in your summary.
- Keep the change minimal and focused on this signature.
- Make sure the project builds. Finish with a short summary of what you
  changed and what you could not do.
"#;

/// Write an agent prompt for the `n`th (1-based) ranked signature.
pub fn workitem(conn: &Connection, cfg: &Config, run_id: &str, n: usize, out: &Path) -> Result<()> {
    let rows = ranked(conn, run_id, n)?;
    let Some(r) = rows.get(n.saturating_sub(1)) else {
        bail!("run {run_id} has only {} signatures", rows.len());
    };
    let detail: Option<String> = conn
        .query_row(
            "SELECT f.detail FROM failures f JOIN runs ru ON ru.id=f.run_row
             WHERE ru.run_id=?1 AND f.signature=?2 AND f.detail!='' LIMIT 1",
            params![run_id, r.signature],
            |row| row.get(0),
        )
        .optional()?;
    let aosp = cfg.agent.as_ref().map_or("(not configured)".into(), |a| {
        a.aosp_frameworks.display().to_string()
    });
    let text = PROMPT
        .replace("{signature}", &r.signature)
        .replace("{kind}", &r.kind)
        .replace("{apps}", &r.apps.to_string())
        .replace("{broken}", &r.broken.to_string())
        .replace("{sole}", &r.sole.to_string())
        .replace("{pkgs}", &r.pkgs)
        .replace("{detail}", detail.as_deref().unwrap_or("(none)"))
        .replace("{aosp}", &aosp);
    fs::write(out, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn interaction_verdicts() {
        use super::verdict;
        use crate::runner::Outcome::*;
        assert_eq!(verdict((Alive, Some(true)), (Alive, Some(false))), -1);
        assert_eq!(verdict((Alive, Some(false)), (Alive, Some(true))), 1);
        assert_eq!(verdict((Alive, None), (Alive, Some(false))), 0);
        assert_eq!(verdict((Alive, Some(true)), (Crash, None)), -1);
        assert_eq!(verdict((Blank, Some(false)), (Alive, Some(false))), 1);
    }

    use super::*;
    use crate::{classify, db};

    fn add(conn: &Connection, run: &str, pkg: &str, outcome: Outcome, log: &str) {
        conn.execute(
            "INSERT INTO runs(run_id, pkg, outcome) VALUES (?1, ?2, ?3)",
            params![run, pkg, outcome.as_str()],
        )
        .unwrap();
        let row = conn.last_insert_rowid();
        for f in classify::parse(log, &[]) {
            conn.execute(
                "INSERT INTO failures VALUES (?1, ?2, ?3, ?4)",
                params![row, f.kind.as_str(), f.signature, f.detail],
            )
            .unwrap();
        }
    }

    #[test]
    fn rank_and_gate() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::connect(dir.path()).unwrap();
        let nsm = "java.lang.NoSuchMethodError: No method foo\n\tat a.B.c(B.java:1)\n";
        add(&conn, "a", "p1", Outcome::Crash, nsm);
        add(&conn, "a", "p2", Outcome::Crash, nsm);
        add(&conn, "a", "p3", Outcome::Alive, "");
        add(&conn, "b", "p1", Outcome::Alive, "");
        add(&conn, "b", "p2", Outcome::Crash, nsm);
        add(&conn, "b", "p3", Outcome::Blank, "");

        let r = ranked(&conn, "a", 5).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].apps, r[0].broken, r[0].sole), (2, 2, 2));
        assert_eq!(r[0].signature, "java.lang.NoSuchMethodError: No method foo");

        assert_eq!(compare(&conn, "a", "b").unwrap(), 1); // p3 regressed
        assert_eq!(compare(&conn, "b", "b").unwrap(), 0);
    }
}
