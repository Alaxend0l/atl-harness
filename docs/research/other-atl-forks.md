<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Other ATL forks and Android-on-Linux projects (survey note)

Notes captured 2026-11-21. There are several ATL forks doing much of what we
do, including AI-assisted work. Surveying them is allowed: our rule (D1 in
docs/decisions.md) only forbids sending anything upstream — reading other
forks and borrowing from them (GPL-3.0, same as ours, with attribution) is
fine.

| Project | What it is | Why it's interesting to us |
|---|---|---|
| Alexa-RR/android_translation_layer | Fork with "a much larger Android framework implementation: AOSP ports, rendering, text and input fixes", aimed at running Waze | Overlaps our biggest open areas (rendering, text); may already have solved some of them |
| mortelil/android_translation_layer + atl-shelf | "Experimental, AI-assisted" fork for Linux phone apps (Immich first), plus an app manager with F-Droid/GitHub sources, updates and launchers | Closest to our setup; has test suites (e.g. a 73-check MediaExtractor suite) and an installer, comparable to our desktop-apps and the installer plans |
| NikuzoKO/android_translation_layer | Fixes to run WhatsApp on a Linux phone, "developed with Claude Code" | Messaging-app compatibility (notifications, media, crypto) |
| killerdevildog/android_translation_layer (and its forks) | Unified CMake build bundling wolfSSL, libunwind, bionic_translation, ART and ATL | Could simplify our runner bootstrap and builds |
| bi0shacker001/android_translation_layer | "droidtop" work toward upstream | Shows what upstream is likely to get next |
| nexumia-de/android_translation_layer | Phosh-specific fork | Mobile-shell integration |
| Lepton (Valve) | Runs Android games on Linux; open-sourced September 2026 | A different architecture, but probably strong on graphics and audio |

## Cautions
- Licensing is compatible. Code from these forks is GPL-3.0, the same as our
  fork, so we may borrow it with attribution. Our rule only forbids sending
  things upstream; learning from forks is fine.
- Several are AI-assisted too. Treat their fixes as leads to verify, not as
  proven: the same plausible-but-wrong risk applies.

## Follow-up (in progress)
One research agent is going through the most relevant forks (Alexa-RR,
mortelil, NikuzoKO, killerdevildog, and Lepton's design), reading their
recent commits and comparing them with our task list. The goal is
`docs/research/atl-forks.md`: per open task (audio, interpolators, text,
canvas, dialogs, widgets, Compose rendering for Flow), whether another fork
already solved it and where — so agents get tested code to study instead of
starting from zero.

Sources:
- Alexa-RR/android_translation_layer
- mortelil/android_translation_layer
- mortelil/atl-shelf
- mortelil PR #77 (MediaExtractor checks)
- NikuzoKO/android_translation_layer
- killerdevildog/android_translation_layer
- bi0shacker001/android_translation_layer
- nexumia-de/android_translation_layer
- Lepton from Valve open-sourced (GamingOnLinux)
- NLnet: Android translation layer
