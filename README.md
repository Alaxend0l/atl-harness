# atlh — ATL compatibility harness

Runs a corpus of F-Droid apps under Android Translation Layer, headless,
classifies why they fail and ranks the failures by impact. Includes a
regression gate and agent-prompt generation for an automated fix loop on a
hard fork of ATL. See [DESIGN.md](DESIGN.md).

Unofficial and AI-assisted; not affiliated with the upstream ATL project.
Please don't report issues from this project upstream.

## Box setup (Linux)
- Rust toolchain (`rustup`, stable). SQLite is bundled; no system libs needed
  beyond a C compiler.
- `Xvfb`, ImageMagick (`import`, `identify`), Mesa (software GL is fine).
  - Debian/Ubuntu: `apt install build-essential xvfb imagemagick mesa-utils`
  - Fedora: `dnf install gcc xorg-x11-server-Xvfb ImageMagick mesa-dri-drivers`
  - Alpine/pmOS: `apk add build-base xvfb imagemagick mesa-dri-gallium`
- ATL built from the fork; check `[atl].command` in `config.toml`.
- Optional, for the agent loop: `opencode` and/or `claude` CLI.

## Usage (from laptop)
```sh
export ATLH_REMOTE=user@box
scripts/remote.sh sync
scripts/remote.sh atlh fetch --limit 50
scripts/remote.sh atlh run --run-id base-1121 --jobs 4     # use tmux for big runs
scripts/remote.sh atlh rank base-1121
scripts/remote.sh atlh workitem base-1121 -n 1             # prompt for top signature
scripts/remote.sh atlh compare base-1121 fix-setelevation  # exit 1 = regression
scripts/remote.sh pull     # db, logs, screenshots back to ./work
```
Or on the box directly: `cargo run --release -- <command>`.

First thing to do on the box: run ~10 apps, read the logs in
`work/runs/<id>/`, and tune `stub_patterns` and `[atl].command` until the
"broken with no signature" count in `rank` is low.

## Development
`cargo test` · `cargo clippy` · `cargo fmt`
