<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# RFC/research: should Andrun be rebuilt in Rust? (2026-11-23, research only — no decision made)

Question from the owner: before we keep building Andrun (the ATL fork)
"almost entirely in Java", would a Rust rebuild be better for (1) long-term
stability, (2) headless testing / Rust test tooling, (3) the ability to
rebuild the project under AGPL, and (4) not betting on a language Google
is abandoning (Kotlin). This document records the measurements, sources,
and options so the decision can be made later without redoing the work.
Per the standing research-task rule: **report, don't adopt.**

## TL;DR (recommendation, not a decision)

A full rewrite in Rust is not supported by the evidence: it does not
remove the Java layer (the binding constraint is **dex bytecode + ART**,
not the source language), it forfeits an unusually productive upstream
(~143 commits/3 months, 8 active devs — all writing Java), and our
root-caused bug history shows the failures are *Android-semantics gaps*,
not Java-caused defects. The two cheap, honest wins available now:
(a) a **JVM unit-test rig** for the pure-logic Java (fixes the headless
testing complaint without a rewrite), and (b) selectively migrating the
**31 k-line C JNI layer** (`api-impl-jni`) to Rust over time — that is
where the real memory-safety exposure is, and it has no dex constraint.
A small **Rust+JNI spike** on one leaf component is the suggested
evidence-gathering step before ever considering (b) as policy.

## 1. What Andrun actually is (measured 2026-11-23)

From `~/Develop/android-translation-layer` (fork main):

| Component | Lang | LOC | Role |
|---|---|---|---|
| `src/api-impl` | Java | **335 915** (915 files, hand-written; only 42 files mention "generated") | the Android SDK surface: view system, widgets, content, providers, graphics… Runs as **dex on ART** (explicit `srcs` in meson; dx constraints: no lambdas/invokedynamic) |
| `src/api-impl-jni` | C | **30 743** (+ generated JNI headers) | GTK4/GSK mirror of every Android view (`WrapperWidget`), EGL, audio (ALSA), JNI natives |
| `src/libandroid` | C | 3 054 | `libandroid.so` shim for native-code apps |
| `src/main-executable` | C | 1 230 | entry, Xvfb/GTK boot |
| `src/test-runner` | Java | 8 055 | `android.test.*` classes (so *apps* can self-test) — not tests of ATL |
| ART, bionic, vixl, OpenSL | C++/Java/mixed | — | built from AOSP/Alpine sources by `rig bootstrap`; **never a rewrite target we own** |
| atl-harness (us) | **Rust** | ~6 k | gates, witnesses, classification — already Rust with cargo tests/clippy; satisfies the "Rust tooling" wish at the integration layer |

Key structural fact: **the Java/C split is already the design.** Java
classes define the dex-visible object model; the C layer implements the
behavior and mirrors it to GTK, calling back into Java virtual methods so
app subclasses keep working (`WrapperWidget.c` `performClick` detection
pattern). "Rewriting in Rust" would keep the Java signature layer (the
~53 k member/declaration surface stays — apps resolve those symbols at
class-load time) and bolt Rust behind it: strictly *more* layers
(Java→Rust JNI×thousands + reverse dispatch into Java overrides), not fewer.

## 2. The dex/ART constraint (why "rewrite the app in Rust" is a category error)

- Apps are **dex**; ART resolves `android.*` classes from the boot
  classpath and dispatches virtual calls between app dex and framework
  dex **inside the Java heap**. Any non-dex implementation must present
  every referenced class/method as dex classes and shuttle every behavior
  call across JNI, including callbacks when apps override methods
  (`onCreate`, `onDraw`, custom views…).
- Google itself, on the question of a Rust app framework (android/ndk
  issue #1742, "Roadmap for supporting Rust in apps"): *"as with C/C++,
  there will be no 'pure' Rust apps"* — not even Google has built a
  parallel framework; nobody has.
- Prior art is unanimous: **no non-dex Android framework has ever worked
  at app level.** `rustdroid` (Rust framework for Android apps, ~2020) is
  abandoned; ChromeOS ARC++, Anbox, Waydroid, BlissOS all **reuse real
  Android** (containers/kernel) instead of rewriting the framework.
  Details in `docs/research/other-atl-forks.md` / `platform-reach.md`
  complement this.
- The **Kotlin** point (owner note): Kotlin is Google's recommended *app*
  language, but it compiles to the same dex and interops 1:1 with the
  Java framework — it is not a move away from the runtime model that
  constrains us. Java/Kotlin are AOSP's two "officially supported memory
  safe languages" for the framework (developer.android.com memory-debug
  guide). Switching api-impl source to Kotlin would add kotlinc +
  kotlin-stdlib to the boot classpath, break against the old `dx` tool
  in use, and make upstream diffs unmergeable — no upside identified.

## 3. What Google is actually doing with Rust (sources, checked 2026-11-23)

- `source.android.com/docs/setup/build/rust/building-rust-modules/overview`:
  Rust is for **native OS components** (the C/C++ tier).
- Google security blog "Rust in Android: move fast and fix things": in
  *systems languages*, Rust is rising and new C++ declining; ~1.5 M
  lines of Rust in AOSP (2025, arXiv greybox-fuzzing paper cites
  "approximately 1.5 million lines of Rust code currently present in
  AOSP") — Bluetooth/NFC stacks etc. That is Andrun's **C tier analog**,
  not its Java tier. ART and the framework remain dex languages.

Conclusion for us: Google's Rust push *supports* migrating our C JNI
layer eventually (same reasoning as AOSP's), and *does not support*
replacing the dex layer.

## 4. The upstream stream (the largest practical cost of a rewrite)

Measured on `upstream/master` (gitlab postmarketOS ATL; fetch-only — D1
keeps push disabled):

- **143 commits in the last 3 months, 195 in 6 months, ~8 active authors
  in 12 months** (Julian Winkler 199, Fox2Code 38, Mis012 29, …). Real
  API work (GradientDrawable theming, TextToSpeech via libspeechd,
  LayoutTransition…).
- Fork `main` diverged from upstream only **2026-11-08** and is **5
  commits behind** — we consume that stream essentially daily.
- Upstream writes 100% Java (api-impl) + C (api-impl-jni). A Rust fork
  permanently stops merging any of it: every API they complete, we would
  then have to implement ourselves in Rust+dex-signatures+JNI glue. For
  a 2-human setup, that cost is the single strongest argument in the
  whole analysis.

## 5. Testing: the complaint is real; the rewrite doesn't fix it

- What we cannot do today: unit-test `api-impl` logic without ART+GTK.
  **But the Java is plain javac-compilable code**; its hooks into the
  native layer are ~116 files with `native` methods. A workstation
  **JVM test rig** (compile api-impl, stub/mock the natives, run JUnit-ish
  tests on the pure-logic parts: view geometry, menu model, event
  dispatch, Intent/Bundle semantics) gets most of the claimed "Rust test
  tooling" benefit at a fraction of the cost. Prototype cost: hours–days.
- What no rewrite changes: the **apps are dex**, so ground-truth testing
  of app behavior stays ART+Xvfb (the `atlh` gates/witnesses). A Rust
  framework would still be validated by exactly these same runs — Rust
  unit tests can only cover Rust code, and the risky part (app
  compatibility) is unconditionally integration-tested.
- The harness (gates, signatures, witnesses, `compare`) is already Rust
  with 25+ unit tests, clippy, cargo — the "use Rust's testing tools"
  wish is 80% satisfied where it matters most.

## 6. Stability: what our bug history actually says

Root causes found this month (all verified): `start_threshold = one
period` in ALSA setup (C), missing `MediaSession.sendSessionEvent`
(Java, API completeness), `AlertDialog.Builder.setView` dropped (C
layout gap), click propagation leaf-only (Java/C seam), marquee/Compose
semantics (missing features). **None is a "Java caused it" bug.** They
are gaps in matching Android's documented/observed behavior. Rust would
not have caught any of these; it would help against GObject/UAF-class
bugs in the 31 k-line C layer — a real but smaller exposure, and exactly
the part that *could* migrate to Rust incrementally (trivial C↔Rust
interop, no dex constraint).

## 7. The AGPL question (facts)

- Upstream ATL is **GPLv3** (`LICENSE.txt`); the fork keeps fork changes
  under GPLv3 (`FORK.md` lines 15–16). The harness is AGPL-3.0-or-later
  (D15).
- GPL-3.0 and AGPL-3.0 are mutually compatible (§13 of AGPL): the fork
  can already distribute our AGPL-labeled contributions combined with
  the GPLv3 base; for a desktop app the AGPL network clause adds nothing
  operationally today (noted already in D15).
- So "rebuild to get AGPL" buys essentially nothing: the only thing a
  clean-room rewrite achieves license-wise is purging GPLv3 lines, which
  matters only if the owner later wants relicensing/dual-licensing
  freedom the GPLv3 base forbids. That is a real (if speculative) long
  game; a **gradual module-by-module replacement** (new code under AGPL,
  old GPL modules rewritten when touched) achieves it without a big bang.

## 8. Options and costs (for the eventual decision)

| | Option | Cost | Risk | Verdict from evidence |
|---|---|---|---|---|
| A | Full rewrite of api-impl (+C layer) in Rust | years (336 k-line surface + full JNI re-plumbing); upstream stream lost day one | fatal | **No** — contradicts §2, §4 |
| B | Migrate `api-impl-jni` (31 k C) to Rust incrementally | months, file-by-file; keeps upstream Java intact; C↔Rust interop easy | medium (glue churn, build chain) | Plausible **later**, per-file; the memory-safety argument applies here and matches Google's own Rust-in-systems-layers policy |
| C | JVM unit-test rig for api-impl (stub natives) | days | low | **Do regardless** — addresses the testing complaint directly |
| D | Spike: one leaf component (e.g. `ATLElementDump` or the audio state machine) rewritten in Rust+JNI, measure glue cost + testability | ~a day | none (additive) | Recommended first evidence step before endorsing B |
| E | Kotlin for api-impl source | kotlinc + stdlib on boot classpath, dx incompatibility, upstream unmergeable | high | No |
| F | Status quo (Java+C, Rust harness) | — | — | Current default; §5–§6 show the Java part is not where the claimed pain comes from |

Suggested decision process (owner-paced): run C (immediately useful),
run D once (empirical glue-cost number), then revisit B with real data.
Re-open the full-rewrite question only if any of these become true:
upstream goes dormant (today: very active); the C tier shows a sustained
memory-corruption bug rate (today: our bugs are semantic gaps); ART/dex
itself becomes the blocker (never observed; ART is not ours to rewrite
anyway).

## 9. What this document deliberately does NOT claim

- Not that Rust is a bad fit *anywhere* here — B/D are real candidates.
- Not that the fork will never change shape; D19-style incremental
  substitution has worked so far.
- Not that upstream's code quality is sufficient — we already diverge
  104 commits; the argument is that *merging their Java is cheaper than
  reimplementing it in Rust*, which holds regardless.

Reproduce the measurements: `find src/api-impl -name '*.java' | xargs wc
-l`, `git fetch upstream master && git rev-list --count FETCH_HEAD
--since="3 months ago"`, `git merge-base main upstream/master`.
