# Agent task: research compatibility layers and tooling

Standing research task, approved by the project owner (2026-11-21). ATL is
to Android what Wine is to Windows; other translation layers have solved
problems this project is about to hit. Find what can be reused, learned
from, or built, and report it so the owner can decide.

## You may
- Search the web and read docs, source code, papers, issue trackers and
  wikis of the projects below and anything they lead to. Budget web search
  per `~/.config/opencode/AGENTS.md` (built-in search first, Tavily only when
  it comes up short).
- Clone and build open-source tools **in your home directory on a runner**
  (`rig sh`), never system-wide; prototype small tools or harness
  subcommands on a branch (`research/<topic>`), with tests like any other code.
- Run tools against **F-Droid apps** in the corpus (they're open source).

## You may not
- Contact anyone: no posting to forums, issue trackers, mailing lists or
  chats of any project, including Wine/Proton/FEX. Nothing goes to upstream
  ATL (AGENTS.md hard rule).
- Copy code into this repo or the ATL fork unless its license allows it
  (GPLv3 fork: GPL-compatible only; LGPL/Apache/MIT/BSD are fine with
  attribution) and the report says so. Never use proprietary components
  (libhoudini, libndk_translation, Google Play Services).
- Decompile or instrument proprietary (non-F-Droid) apps.
- Install anything outside home directories, touch the phone beyond
  `rig` and its safety rules, or spend money (no paid APIs or services).
- Adopt a tool into the main workflow: propose it; the owner decides.

## Topics, and the question for each

**Translation layers (what to learn)**
- **Wine / Proton**: architecture (PE loader, wineserver, builtin vs
  native DLLs, `WINEDEBUG` fixme channels ≈ ATL's stub logs). How Wine
  tests conformance (its test suite, winetest, test.winehq.org) and how
  Valve/ProtonDB measure compatibility at scale (ratings, Steam Deck
  Verified, automated game testing). What maps onto our harness, gate and
  ranking?
- **DXVK / vkd3d-proton**: graphics API translation. Lessons for ATL's
  GLES/Vulkan/SurfaceFlinger paths?
- **Darling** (macOS on Linux), **Anbox / Waydroid**, **libhybris**
  (bionic on glibc, vs. ATL's bionic_translation), **Shashlik**: what worked,
  what killed them, what's reusable.
- **Robolectric**: real Android framework classes on a host JVM with
  "shadows". Could its shadows or approach fill ATL gaps (license: MIT)?

**Binary translation (the ARM-only app gap)**
- **FEX-Emu, box64, Rosetta 2, QEMU user mode**: x86 on ARM and vice
  versa. Which could run ARM-only native libs on the x86_64 runner, or
  x86-only libs on aarch64, inside ATL's process? Performance and
  correctness costs?
- **Berberis** (AOSP's open-source native bridge, Apache-2.0): what guests
  and hosts it supports today, and whether it can serve as ATL's native
  bridge for arm64 apps on x86_64.

**Tools that would help this project**
- **API coverage map**: compare ATL's `api-impl` against AOSP's public API
  (`current.txt`), and scan corpus APKs' dex references (dexdump, baksmali,
  androguard) to predict which missing APIs block the most apps before
  running them. A prototype here is probably the highest-value output.
- **Screenshot oracles**: SSIM/perceptual diff (ImageMagick `compare`,
  pixelmatch), OCR (tesseract) to check that text actually rendered,
  detecting stray windows (e.g. the GTK file chooser over Fossify Gallery,
  black rectangles over File Manager).
- **Input scripting**: xdotool/ydotool on Xvfb, uiautomator-style scripts,
  to get past first screens (roadmap step 10).
- **Bisection**: `git bisect run` driven by `rig atlh run --pkg …` for
  regressions between fork commits.
- **Native debugging**: strace, ltrace, bpftrace, gdb with
  bionic_translation's tips, for crashes inside native libs (Flutter's
  `library_loader.cc` abort, Godot's missing `ACameraMetadata_*`).
- **Sandboxing**: bubblewrap patterns from Flatpak and Steam's
  pressure-vessel (Proton's runtime container), for roadmap step 4.

## Output
- One file per topic in `docs/research/<topic>.md`: what it is, license,
  status/maturity, relevance to this project (1–5), concrete uses with
  effort estimates, risks, and links to the sources you read. Mark
  anything you didn't verify.
- Prototypes on `research/<topic>` branches, never merged by the agent.
- A summary in `docs/research/README.md`, ranked by value/effort, with
  proposed roadmap entries for the owner to accept or reject.
