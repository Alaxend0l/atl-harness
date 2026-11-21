<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Licensing

Unless a file says otherwise, everything in this repository (the `atlh`
harness, `scripts/`, configuration, corpora lists, documentation) is licensed
under the **GNU Affero General Public License v3.0 or later**
(`AGPL-3.0-or-later`), full text in [LICENSE](LICENSE).

Copyright © 2026 the atl-harness contributors.

New files: start with an `SPDX-License-Identifier: AGPL-3.0-or-later` line
(comment syntax of the file's language).

## Exceptions and third-party material

| What | License | Notes |
|---|---|---|
| `scripts/runner/patches/vixl-0001-meson-support.patch` | as distributed by Alpine aports for vixl (BSD-3-Clause project) | Unmodified copy (sha512 as in Alpine's APKBUILD); applied to vixl during `rig bootstrap`. |
| Short code quotes from the ATL fork in `docs/audit/` | GPL-3.0 (ATL's license) | Quoted for review purposes. |
| Rust dependencies (`Cargo.lock`) | MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, Unlicense, CDLA-Permissive-2.0 (permissive) | Checked 2026-11-21 with `cargo metadata`; all compatible with AGPL-3.0. SQLite (bundled by rusqlite) is public domain. |

## Not covered
- **The ATL fork** (`Alaxend0l/android-translation-layer`) is a separate
  repository under upstream ATL's **GPL-3.0**; this repository's license does
  not apply to it, and upstream code there can't be relicensed. Contributions to
  the fork stay GPL-3.0.
- Android apps the harness downloads (F-Droid, GitHub releases) keep their own
  licenses; they are fetched to `work/` (git-ignored) and never committed.
- Runner root filesystems built by `rig rootfs` contain Fedora packages under
  their own licenses.
