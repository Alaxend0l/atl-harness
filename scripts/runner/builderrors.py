#!/usr/bin/env python3
"""Summarise errors in a build log (gcc/clang, javac, ld, meson, ninja, make,
cargo, dx/d8, dnf, pkg-config).

Usage: builderrors.py LOG [--json] [--max N]
Exit code: 0 if no errors were found, 1 otherwise. Stdlib only, so it runs on
any runner (and inside a bwrap rootfs).
"""
from __future__ import annotations

import json
import re
import sys
from dataclasses import asdict, dataclass, field

ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


@dataclass
class Issue:
    kind: str            # c, java, link, meson, ninja, make, cargo, dex, pkg
    message: str
    file: str = ""
    line: int = 0
    context: list[str] = field(default_factory=list)

    def key(self) -> tuple:
        return (self.kind, self.file, self.line, self.message)

    def where(self) -> str:
        return f"{self.file}:{self.line}" if self.file and self.line else self.file


# (kind, regex). Order matters: the first match on a line wins.
PATTERNS: list[tuple[str, re.Pattern]] = [
    ("c", re.compile(r"^(?P<file>[^\s:][^:]*\.(?:c|cc|cpp|cxx|h|hpp|S)):(?P<line>\d+):(?:\d+:)? (?:fatal )?error: (?P<msg>.+)")),
    ("java", re.compile(r"^(?P<file>\S+\.java):(?P<line>\d+): error: (?P<msg>.+)")),
    ("link", re.compile(r"(?P<msg>undefined reference to [`'][^`']+')")),
    ("link", re.compile(r"(?:/usr/bin/)?(?:ld|ld\.bfd|ld\.gold|ld\.lld|mold)(?:: |: error: )(?P<msg>cannot find .+)")),
    ("link", re.compile(r"collect2: error: (?P<msg>.+)")),
    ("meson", re.compile(r"^(?P<file>\S*meson\.build):(?P<line>\d+):\d+: ERROR: (?P<msg>.+)")),
    ("meson", re.compile(r"^(?:meson\.build:\d+:\d+: )?ERROR: (?P<msg>.+)")),
    ("pkg", re.compile(r"(?P<msg>Package '?[\w.+-]+'?,? .*not found.*|Dependency \"[^\"]+\" not found.*)")),
    ("pkg", re.compile(r"(?P<msg>No match for argument: .+)")),
    ("cargo", re.compile(r"^error(?:\[E\d+\])?: (?P<msg>.+)")),
    ("dex", re.compile(r"^(?:UNEXPECTED TOP-LEVEL EXCEPTION|(?:Error|ERROR) in .+|Compilation failed.*)(?P<msg>.*)")),
    ("ninja", re.compile(r"^FAILED: (?P<msg>.+)")),
    ("make", re.compile(r"make(?:\[\d+\])?: \*\*\* (?P<msg>\[.+\] Error \d+|No rule to make target .+)")),
]
CARGO_LOC = re.compile(r"^\s*--> (?P<file>[^:]+):(?P<line>\d+):\d+")
WARNING = re.compile(r": warning: |^warning: ")
# Follow-on noise that only repeats an error already reported.
NOISE = re.compile(r"ninja: build stopped|make(\[\d+\])?: \*\*\* Waiting for unfinished jobs|"
                   r"error: could not compile|compilation terminated\.|Error: build failed")


def parse(text: str, context: int = 3) -> tuple[list[Issue], int]:
    lines = [ANSI.sub("", ln.rstrip("\n")) for ln in text.splitlines()]
    issues: list[Issue] = []
    seen: set[tuple] = set()
    warnings = 0
    for i, ln in enumerate(lines):
        if WARNING.search(ln):
            warnings += 1
            continue
        if NOISE.search(ln):
            continue
        for kind, rx in PATTERNS:
            m = rx.search(ln)
            if not m:
                continue
            gd = m.groupdict()
            msg = (gd.get("msg") or ln).strip() or ln.strip()
            issue = Issue(kind, msg[:300], gd.get("file") or "", int(gd.get("line") or 0))
            if kind == "cargo":
                for nxt in lines[i + 1:i + 4]:
                    loc = CARGO_LOC.match(nxt)
                    if loc:
                        issue.file, issue.line = loc["file"], int(loc["line"])
                        break
            issue.context = [c for c in lines[i + 1:i + 1 + context] if c.strip()]
            if issue.key() not in seen:
                seen.add(issue.key())
                issues.append(issue)
            break
    # Generic step failures (ninja/make) only matter if nothing more specific was found.
    specific = [x for x in issues if x.kind not in ("ninja", "make")]
    return (specific or issues), warnings


def render(issues: list[Issue], warnings: int, limit: int) -> str:
    out = [f"{len(issues)} error(s), {warnings} warning(s)"]
    for n, it in enumerate(issues[:limit], 1):
        where = f" {it.where()}" if it.where() else ""
        out.append(f"  {n}. [{it.kind}]{where}: {it.message}")
        out.extend(f"       | {c[:200]}" for c in it.context)
    if len(issues) > limit:
        out.append(f"  ... {len(issues) - limit} more")
    return "\n".join(out)


def main(argv: list[str]) -> int:
    args = [a for a in argv if not a.startswith("--")]
    limit = 15
    if "--max" in argv:
        limit = int(argv[argv.index("--max") + 1])
        args = [a for a in args if a != str(limit)]
    if not args:
        print(__doc__, file=sys.stderr)
        return 2
    with open(args[0], errors="replace") as f:
        issues, warnings = parse(f.read())
    if "--json" in argv:
        print(json.dumps({"errors": [asdict(i) for i in issues], "warnings": warnings}, indent=1))
    else:
        print(render(issues, warnings, limit))
    return 1 if issues else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
