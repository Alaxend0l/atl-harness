<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# RFC/research: should Andrun be rebuilt in Rust? (2026-11-23, research only — no decision made)

Question from the owner: before we keep building Andrun (the ATL fork)
"almost entirely in Java", would a Rust rebuild be better for (1) long-term
stability, (2) headless testing / Rust test tooling, (3) the ability to
rebuild the project under AGPL, and (4) not betting on a language Google
is abandoning (Kotlin). This document records the measurements, sources,
and options so the decision can be made later without redoing the work.
Per the standing research-task rule: **report, don't adopt.**

## TL;DR (recommendation, not a decision — see the premise correction below before weighing §4)

**Premise correction (owner clarification, same day, D20):** Andrun and
upstream ATL will **never share code again** (ideological separation over
AI use); upstream merges are repealed (D20). This *removes* the "upstream
stream" argument from §4 — the rest of the analysis was re-derived on
purely technical grounds and the conclusion did not change. What it does
change: restructuring the fork freely is now unconstrained, which makes
the incremental options (B, D) cheaper in friction, and makes the
"surface work" comparison fairer: we must implement every API ourselves
eventually either way, so the rewrite question reduces to the *marginal*
cost of Rust+JNI glue vs Java per component — measurable, see Option D.

Full rewrite in Rust remains unsupported by the evidence: it does not
remove the Java layer (the binding constraint is **dex bytecode + ART**,
not the source language — a Rust framework keeps the whole dex-visible
signature surface and *adds* Rust+glue underneath), no one has ever
succeeded at it (§2), and our root-caused bug history shows the failures
are *Android-semantics gaps*, not Java-caused defects — the Java tier is
already GC/memory-safe; Rust's stability margin is real only over the
**C tier** (§6). The two cheap, honest wins available now: (a) a **JVM
unit-test rig** for the pure-logic Java (fixes the headless testing
complaint without a rewrite), and (b) selectively migrating the
**31 k-line C JNI layer** (`api-impl-jni`) to Rust — that is where the
real memory-safety exposure is, and it has no dex constraint. A small
**Rust+JNI spike** on one leaf component is the suggested evidence-gathering
step before making (b) policy.

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

## 4. ~~The upstream stream~~ — WITHDRAWN as an argument (D20, same day)

*The original §4 measured the value of merging upstream (`143 commits in
3 months, 8 authors, fork 5 commits from base`) and argued a Rust rewrite
would forfeit that stream. The owner then clarified that ATL and Andrun
will never share code again — the hard fork is complete (D20), upstream
merges are repealed. The number above stays only as history of the fork
state at measurement time; it no longer weighs on anything.*

What the severance does and doesn't change for the rewrite question:

- **Removes**: "you lose upstream's Java work for free" (never applicable
  again) — and correspondingly the "upstream diffs become unmergeable"
  objection to restructuring, Kotlin experiments, or Rust migration
  (options B/D/E were partly disqualified by sync damage; that part is
  void).
- **Neutral**: the API-surface workload. We implement every needed API
  ourselves either way; the question reduces to the *marginal* cost and
  benefit of Rust+JNI glue vs Java per component (measurable — Option D).
- **Strengthens slightly**: incremental Rust migration of the C tier and
  free architectural restructuring (no upstream shape to preserve).
- **Does not touch**: the dex/ART constraint (§2), the 336 k-line
  dex-visible surface (§1), prior art (§2), the bug-class evidence (§6),
  the license facts (§7) — all independent of upstream.
- One residual, knowledge-only: upstream's public history documents
  Android behaviours they already got right (marquee, ScrollView, AOSP
  quirks). Whether even *reading* it (without copying code) is allowed
  under the ideological separation is **pending the owner's ruling**;
  agents should not consult it meanwhile. This is a research-permission
  question, not a code-sharing one, and it does not affect any option's
  technical assessment.

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
| A | Full rewrite of api-impl (+C layer) in Rust | years (336 k-line surface, all ours to hand either way, plus a full JNI re-plumbing — the dex signature layer stays *on top* of the Rust) | fatal | **No** — rests now on §1–§2 alone: dex contract, surface, zero prior art, and no stability prize in the Java tier (it is GC-managed) |
| B | Migrate `api-impl-jni` (31 k C) to Rust incrementally | months, file-by-file; C↔Rust interop easy; D20 removed any sync objection | medium (glue churn, build chain) | **Plausible** — the memory-safety argument applies exactly here (matches Google's own Rust-in-the-native-tier policy); gate adoption on D's measurement |
| C | JVM unit-test rig for api-impl (stub natives) | days | low | **Do regardless** — addresses the testing complaint directly |
| D | Spike: one leaf component (e.g. `ATLElementDump` or the audio state machine) rewritten in Rust+JNI, measure glue cost + testability | ~a day | none (additive) | Recommended first evidence step for B |
| E | Kotlin for api-impl source | kotlinc + kotlin-stdlib on boot classpath, old dx toolchain | high | Still **no** (the upstream-merge objection is gone, the toolchain/runtime ones stand) |
| F | Status quo (Java+C, Rust harness) | — | — | Current default; §5–§6 show the Java part is not where the claimed pain comes from |

Suggested decision process (owner-paced): run C (immediately useful),
run D once (empirical glue-cost number), then revisit B with real data.
Re-open the full-rewrite question only if any of these become true:
the C tier shows a sustained memory-corruption bug rate (today: our bugs
are semantic gaps); ART/dex itself becomes the blocker (never observed;
ART is not ours to rewrite anyway); or harness data starts showing the
Java tier dominating defect rates (today: it does not — see §6).

## 9. What this document deliberately does NOT claim

- Not that Rust is a bad fit *anywhere* here — B/D are real candidates.
- Not that the fork will never change shape; D19-style incremental
  substitution has worked so far.
- Not that upstream's code quality or relationship were ever the reason
  to stay in Java — after D20 they are irrelevant; the conclusion rests
  solely on the technical sections (§1–§3, §5–§6).

Reproduce the measurements: `find src/api-impl -name '*.java' | xargs wc
-l`, `git fetch upstream master && git rev-list --count FETCH_HEAD
--since="3 months ago"`, `git merge-base main upstream/master`.
