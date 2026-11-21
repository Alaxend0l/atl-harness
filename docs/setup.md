# Setup

## Machine roles

| Machine | Role | Needs |
|---|---|---|
| **Workstation** (Linux) | Holds the git clone; you edit, commit and drive the box from here. | git, ssh, rsync; optionally Rust to run `cargo test` locally |
| **Runner box** (Linux, on the LAN, reachable over SSH) | Builds and runs ATL; runs the harness and later the coding agent. Stores the corpus, logs and database. | Rust, Xvfb, ImageMagick, Mesa, the ATL fork built from source |

You can run everything on one machine too: the harness doesn't care, and
`scripts/remote.sh` is just a convenience for the two-machine setup.

## Workstation

```sh
git clone git@github.com:Alaxend0l/atl-harness.git
cd atl-harness
cargo test          # optional; unit tests run without Xvfb/ATL
```

Set up SSH to the box with a key and an alias in `~/.ssh/config`:

```
Host atl-box
    HostName <box-ip-or-hostname>
    User <you>
```

Then point the helper script at it, e.g. in your shell profile:

```sh
export ATLH_REMOTE=atl-box
# export ATLH_REMOTE_DIR=atl-harness   # remote dir, relative to ~ (default)
```

`scripts/remote.sh sync` copies your working tree (including uncommitted
changes, excluding `work/`, `target/`, `.git/`) to the box. If you'd rather
keep the box on git, clone the repo there instead and `git pull`; both work.

## Runner box

### System packages
- Debian/Ubuntu: `sudo apt install build-essential xvfb imagemagick mesa-utils rsync tmux`
- Fedora: `sudo dnf install gcc xorg-x11-server-Xvfb ImageMagick mesa-dri-drivers rsync tmux`
- Arch: `sudo pacman -S base-devel xorg-server-xvfb imagemagick mesa rsync tmux`

### Rust
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
SQLite is bundled into the binary, so no `libsqlite3-dev` is needed.

### ATL
Build ATL from source following the upstream project's build instructions
(not done yet; see [roadmap](roadmap.md)). Once the fork repository
exists, build from the fork instead. Make sure the binary is on `PATH` or
put its absolute path in `[atl].command` in `config.toml`.

### Smoke test
```sh
# on the workstation
scripts/remote.sh sync
scripts/remote.sh atlh --help      # first call compiles a release build (a few minutes)
```

## First-run checklist

Everything here was written before ATL was run on the box, so these
assumptions need confirming. Tick them off here as you go.

- [ ] **CPU architecture.** `uname -m` on the box. `aarch64` → keep
      `allowed_abis = ["arm64-v8a"]`; `x86_64` → set `["x86_64"]` and expect
      fewer eligible apps with native code (most ship ARM only).
- [ ] **ATL command line.** Find the binary name and flags of your build and
      set `[atl].command`. Check whether some apps need an explicit launch
      activity flag.
- [ ] **ATL renders under Xvfb.** Run one known-good app by hand:
      ```sh
      Xvfb :50 -screen 0 1080x1920x24 &
      DISPLAY=:50 GDK_BACKEND=x11 <atl command> some.apk
      import -display :50 -window root /tmp/shot.png
      ```
      If it fails on GL, try `LIBGL_ALWAYS_SOFTWARE=1` (add to `[atl.env]`).
      If ATL needs Wayland, the runner needs a headless compositor backend
      instead (`weston --backend=headless`); see roadmap.
- [ ] **Stub log format.** Read a few logs in `work/runs/<id>/` and update
      `[classify].stub_patterns` to match what ATL prints for unimplemented
      methods.
- [ ] **Classifier coverage.** After a 10–20 app run, `rank` prints
      "broken with no signature". Read those apps' logs and add patterns
      or classifier rules until that number is low.
- [ ] **Blank threshold.** Compare `screen_stddev` of visibly blank vs.
      working screenshots and adjust `blank_stddev_threshold`.
- [ ] **Agent CLI flags** (only when starting the agent loop): confirm the
      non-interactive invocation of opencode / claude and set `[agent]`.
