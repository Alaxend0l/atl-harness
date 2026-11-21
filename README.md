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
| [HANDOFF.md](HANDOFF.md) | Current state, open items, lessons (read first) |
| [docs/opencode-strata.md](docs/opencode-strata.md) | Continuing with opencode + the local strata model |
| [docs/rig.md](docs/rig.md) | Runner tool: sync, run, smoke, jobs, add/bootstrap machines |
| [AGENTS.md](AGENTS.md) | Context and rules for coding agents |

## Quick start (from the workstation)
```sh
scripts/rig list                            # runners from runners.toml
scripts/rig doctor                          # is the default runner healthy?
scripts/rig atlh fetch --limit 20
scripts/rig atlh run --run-id first
scripts/rig atlh rank first
scripts/rig -r mini smoke some.apk          # one app on the aarch64 runner
```

## Development
`cargo fmt && cargo clippy --all-targets && cargo test`

## License
AGPL-3.0-or-later; see [LICENSE](LICENSE) and [LICENSING.md](LICENSING.md). The ATL fork is a separate repository under GPL-3.0.
