# SPDX-License-Identifier: AGPL-3.0-or-later
# JVM test rig (docs/agent-tasks/jvm-test-rig.md, option C of
# docs/research/rust-rewrite.md): compile the fork's api-impl Java on a
# workstation/runner JVM (no ART, no GTK, no Xvfb) and run headless tests
# of its pure-logic classes, with tiny JNI shims for libcore natives.
#
# Usage:  tools/jvmrig/jvmrig.sh [build|test]
#   ATL_SRC=/path/to/andrun-checkout  (default ~/src/android_translation_layer)
#   WORK=/where/to/build              (default ~/jvmrig)
# See tools/jvmrig/README.md.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
ATL_SRC=${ATL_SRC:-$HOME/src/android_translation_layer}
WORK=${WORK:-$HOME/jvmrig}
CORE_JAR=${CORE_JAR:-/usr/local/lib64/java/core-all_classes.jar}   # art-standalone libcore (ATL's own bootclasspath jar)
mkdir -p "$WORK/lib" "$WORK/tests-out"

echo "== [1/5] filter libcore jar (drop java.* — the JVM provides those)"
python3 - "$CORE_JAR" "$WORK/lib/core-nonjava.jar" <<'PY'
import zipfile, sys
src, dst = sys.argv[1], sys.argv[2]
KEEP = ('android/', 'com/', 'dalvik', 'libcore/', 'icu/', 'org/')
with zipfile.ZipFile(src) as z, zipfile.ZipFile(dst, 'w', zipfile.ZIP_DEFLATED) as o:
    n = 0
    for i in z.infolist():
        if i.filename.startswith('META-INF'):
            continue
        if i.filename.startswith(KEEP):
            o.writestr(i, z.read(i.filename)); n += 1
print('kept', n, 'entries')
PY

echo "== [2/5] copy api-impl to build tree + apply rig-local patches"
rm -rf "$WORK/tree"; cp -r "$ATL_SRC/src/api-impl" "$WORK/tree"
cp "$HERE/RigFd.java" "$WORK/tree/android/atl/rig/RigFd.java" 2>/dev/null || { mkdir -p "$WORK/tree/android/atl/rig"; cp "$HERE/RigFd.java" "$WORK/tree/android/atl/rig/RigFd.java"; }
python3 - "$WORK/tree" <<'PY'
import re, sys, pathlib
root = pathlib.Path(sys.argv[1])
# libcore extends a handful of JDK classes with internal methods; the rig
# compiles against the stock JVM java.*, so reroute those calls to RigFd /
# stock equivalents. Rig-local: these patches NEVER go into the fork.
pats = [
    (re.compile(r'\bdatagramSocket\.getFileDescriptor\$\(\)'), 'android.atl.rig.RigFd.dgramFd(datagramSocket)'),
    (re.compile(r'\b(\w+)\.getFileDescriptor\$\(\)'), r'android.atl.rig.RigFd.socketFd(\1)'),
    (re.compile(r'\b(\w+)\.getInt\$\(\)'), r'android.atl.rig.RigFd.getInt(\1)'),
    (re.compile(r'\b(\w+)\.setInt\$\(([^)]*)\)'), r'android.atl.rig.RigFd.setInt(\1, \2)'),
]
for p in root.rglob('*.java'):
    s = t = p.read_text()
    for rx, rep in pats:
        t = rx.sub(rep, t)
    if t != s:
        p.write_text(t); print('patched', p.relative_to(root))
for rel, subs in [
    ('android/app/Instrumentation.java', [
        ('Thread.setUncaughtExceptionPreHandler(new ExceptionHandler());', 'Thread.setDefaultUncaughtExceptionHandler(new ExceptionHandler());'),
        ('Thread.getUncaughtExceptionPreHandler().uncaughtException', '(Thread.getDefaultUncaughtExceptionHandler()==null?new ExceptionHandler():Thread.getDefaultUncaughtExceptionHandler()).uncaughtException')]),
    ('android/atl/ATLUncaught.java', [('t.dispatchUncaughtException(e)', 't.getUncaughtExceptionHandler().uncaughtException(t, e)')]),
    ('android/util/LruCache.java', [('map.eldest()', 'map.entrySet().iterator().next()')]),
]:
    p = root / rel
    s = t = p.read_text()
    for a, b in subs:
        t = t.replace(a, b)
    if t == s:
        sys.exit(f'rig patch failed to match anything in {rel} — the fork changed this code, update tools/jvmrig patches')
    p.write_text(t); print('patched', rel)
PY

echo "== [3/5] build source list from meson (single source of truth)"
python3 - "$ATL_SRC" "$WORK" <<'PY'
import re, sys
atl, work = sys.argv[1], sys.argv[2]
src = open(f"{atl}/src/api-impl/meson.build").read()
files = re.findall(r"'([^']+\.java)'", re.search(r'srcs\s*=\s*\[(.*?)\]', src, re.S).group(1))
lines = [f"{work}/tree/{f}" for f in files]
lines.append(f"{work}/tree/android/atl/rig/RigFd.java")
open(f"{work}/srcs.txt", 'w').write('\n'.join(lines) + '\n')
print('srcs:', len(lines))
PY

echo "== [4/5] javac the framework + tests"
rm -rf "$WORK/classes"; mkdir -p "$WORK/classes"
javac -nowarn -source 8 -target 8 -encoding UTF-8 \
  -classpath "$WORK/lib/core-nonjava.jar" \
  -d "$WORK/classes" @"$WORK/srcs.txt"
TESTS=$(find "$HERE/tests" -name '*.java')
javac -nowarn -source 8 -target 8 \
  -classpath "$WORK/classes" -d "$WORK/tests-out" $TESTS
TESTCLASSES=$(cd "$WORK/tests-out" && find . -name '*.class' | sed 's|^\./||; s|\.class$||; s|/|.|g')

echo "== [5/5] compile JNI shims, run tests on the JVM"
rm -f "$WORK/libatlshim.so"
JH=$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")
gcc -shared -fPIC -I"$JH/include" -I"$JH/include/linux" "$HERE/shim.c" -o "$WORK/libatlshim.so"
FAIL=0
for T in $TESTCLASSES; do
  if java --add-opens java.base/java.io=ALL-UNNAMED --add-opens java.base/java.net=ALL-UNNAMED \
       -Dshim="$WORK/libatlshim.so" \
       -cp "$WORK/tests-out:$WORK/classes:$WORK/lib/core-nonjava.jar" "$T"; then
    echo "PASS $T"
  else
    echo "FAIL $T"; FAIL=1
  fi
done
exit $FAIL
