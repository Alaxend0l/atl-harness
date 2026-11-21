// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS apps(
  pkg TEXT PRIMARY KEY,
  version_code INTEGER,
  version_name TEXT,
  apk_path TEXT NOT NULL,
  native_abis TEXT,
  min_sdk INTEGER,
  added_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS runs(
  id INTEGER PRIMARY KEY,
  run_id TEXT NOT NULL,
  pkg TEXT NOT NULL,
  outcome TEXT NOT NULL,
  exit_code INTEGER,
  duration_s REAL,
  screen_stddev REAL,
  log_path TEXT,
  screenshot_path TEXT,
  started_at TEXT DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(run_id, pkg)
);
CREATE TABLE IF NOT EXISTS failures(
  run_row INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  signature TEXT NOT NULL,
  detail TEXT
);
CREATE INDEX IF NOT EXISTS failures_sig ON failures(signature);
CREATE INDEX IF NOT EXISTS failures_run ON failures(run_row);
";

/// Columns added after the first release; ALTER existing databases in place.
fn migrate(conn: &Connection) -> Result<()> {
    let cols: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('runs')")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    for (name, ty) in [("interact_ok", "INTEGER"), ("interact_detail", "TEXT")] {
        if !cols.iter().any(|c| c == name) {
            conn.execute_batch(&format!("ALTER TABLE runs ADD COLUMN {name} {ty}"))?;
        }
    }
    Ok(())
}

pub fn connect(work_dir: &Path) -> Result<Connection> {
    let conn = Connection::open(work_dir.join("atlh.sqlite"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(SCHEMA)?;
    migrate(&conn)?;
    Ok(conn)
}
