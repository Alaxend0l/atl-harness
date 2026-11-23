<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: JVM unit-test rig for api-impl (D21 option C)

Status: **done (core) 2026-11-23** — v0 + shim generator + test targets all
verified on the workstation (JDK 25.0.5) against fork `main` `4a97b41b`.
Optional item 4 (`atlh jvmrig` wrapper) not done. Next per D21: the option D
spike (Rust+JNI leaf) is unblocked.

## Goal

Headless unit tests of the fork's `api-impl` pure-logic Java on a stock JVM:
no ART, no GTK, no Xvfb, no runner. This is option C of
`docs/research/rust-rewrite.md` (owner decision D21: do C, then the D spike;
B conditional on D).

## v0 — what works now

`tools/jvmrig/jvmrig.sh test` (ATL_SRC=... WORK=... CORE_JAR=... overridable):
1. Filters art-standalone's `core-all_classes.jar` to drop `java.*`
   (the JVM provides those; keeping libcore's would force its typed
   `System.arraycopy` overloads → `NoSuchMethodError` at runtime).
2. Copies `src/api-impl` to a work tree and applies **rig-local** patches —
   libcore extensions of JDK classes that stock `java.*` lacks:
   `getInt$/setInt$/getFileDescriptor$` → `RigFd` reflection helper
   (`--add-opens java.base/java.io,java.net`), `Thread` uncaught-handler
   extensions → stock equivalents, `LinkedHashMap.eldest()` → iterator.
   Patches fail loudly if the fork's code no longer matches. **Never merge
   these patches into the fork.**
3. Builds the file list from `src/api-impl/meson.build`'s `srcs` (855 files
   = single source of truth).
4. `javac` everything (JDK 25, `-source/-target 8`) — verified green.
5. Compiles `shim.c` (JNI natives ART provides but HotSpot cannot:
   currently `dalvik.system.VMRuntime.newUnpaddedArray`; note HotSpot
   receives component types ("int") where ART passes array types ("[I") —
   the shim accepts both), loads it via `-Dshim=…`, and runs every class in
   `tools/jvmrig/tests/` whose `main` asserts (PASS/FAIL each).
   v0 test: `TextAndUriTest` PASS (SpannableStringBuilder + spans + Uri).

## Remaining work

1. ~~Workstation home~~ **done 2026-11-23**: owner installed
   `java-25-openjdk-devel`. The rig now runs fully on the workstation:
   `CORE_JAR=work/core-all_classes.jar ATL_SRC=~/Develop/android-translation-layer WORK=/tmp/opencode/jvmrig bash tools/jvmrig/jvmrig.sh test`
   (jar copied once from bazzite: `scripts/rig -r bazzite sh 'cp -f
   /usr/local/lib64/java/core-all_classes.jar ~/'` + scp into `work/`;
   verified PASS with JDK 25.0.5, 856 srcs on the workstation clone).
2. **Shim generator** — **done 2026-11-23**: `tools/jvmrig/genshim.py` parses
   every `native` declaration in the tree (modifiers, arrays, varargs,
   import-resolved descriptors, JNI long-form mangling for overloads:
   `_1`/`_3`/`_`/hex escapes) and emits **1029 recording stubs**
   (`RIG-NATIVE <symbol>` to stderr, zero-value returns) compiled into the
   same `.so` as the hand-written shims. Inner-class natives get the file's
   top-level class (unresolved, never wrong data).
3. **Test targets** — **done 2026-11-23**: 6 test classes, all PASS on
   `main` `4a97b41b`: `TextAndUriTest` (v0), `UriMoreTest` (hierarchical +
   opaque Uris, Builder round-trip, encode/decode, query edge cases),
   `IntentFilterMatchTest` (action/scheme matching, AOSP MATCH_* codes;
   asserts category/type matching as a known non-feature),
   `AnimEvaluatorsTest` (ArgbEvaluator singleton + RGB-mode midpoints,
   FloatEvaluator), `SpannedGapTest` (span shift/absorb/drop semantics —
   the fork is already AOSP-correct here), `KeyEventMetaTest` (obtain/copy/
   normalize/hasModifiers/changeAction/changeFlags).
   **Rig finding #1:** `KeyEvent.changeTimeRepeat` misses AOSP's
   `repeat != 0 ⇒ FLAG_LONG_PRESS` rule (its `new KeyEvent(orig,time,repeat)`
   path skips the `mFlags` update) — flagged warn-only
   (`RIG-KNOWN-GAP`) in the test; fork-fix candidate for the queue.
4. Optional: `atlh jvmrig` wrapper subcommand; keep unit tests network/Xvfb-free.

## Constraints

- Rig-local patches must stay rig-local (the fork must keep compiling for
  ART exactly as-is).
- Ground truth for real apps remains ART+Xvfb gates/witnesses; the JVM rig
  complements, never replaces them.

## Runner use

bazzite for the v0 probe+verification (`scripts/rig -r bazzite sh 'bash
tools/jvmrig/jvmrig.sh test'`); nothing installed to /usr/local.
