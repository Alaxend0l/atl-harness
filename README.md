# atlh: ATL compatibility harness

Runs a corpus of F-Droid apps under Android Translation Layer (ATL),
headless, classifies why they fail, and ranks the failures by how many apps
they block. It also has a regression gate and agent-prompt generation, the
building blocks for an automated fix loop on a **hard fork** of ATL.

> Unofficial and AI-assisted; not affiliated with the upstream ATL project.
> Don't report issues from this project upstream.

**Status:** early. Census pipeline written and unit-tested, not yet run
against real ATL. See [docs/roadmap.md](docs/roadmap.md).

## Docs
| Doc | For |
|---|---|
| [docs/setup.md](docs/setup.md) | Workstation + runner box setup, first-run checklist |
| [docs/usage.md](docs/usage.md) | Commands, typical loop, data layout, config reference |
| [docs/architecture.md](docs/architecture.md) | Code map, signatures, design constraints |
| [docs/design.md](docs/design.md) | The full plan: stages, hard fork, reward hacking |
| [docs/decisions.md](docs/decisions.md) | Why things are the way they are |
| [docs/roadmap.md](docs/roadmap.md) | Status and next steps |
| [AGENTS.md](AGENTS.md) | Context and rules for coding agents |

## Quick start (from the workstation)
```sh
export ATLH_REMOTE=atl-box                  # ssh alias of the runner box
export ATLH_REMOTE_BOX=atl                  # distrobox on the box (omit if none)
scripts/remote.sh sync
scripts/remote.sh atlh fetch --limit 20
scripts/remote.sh atlh run --run-id first --jobs 2
scripts/remote.sh atlh rank first
```

## Development
`cargo fmt && cargo clippy --all-targets && cargo test`
