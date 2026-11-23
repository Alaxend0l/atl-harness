#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Guest-side audio proof for the VZ virtio-sound spike.
echo "kernel: $(uname -r)"
sudo modprobe virtio_snd && echo MODPROBE_OK || echo MODPROBE_FAIL
aplay -l 2>&1 | head -4
python3 - <<'PY'
import wave, struct, math
w = wave.open("/tmp/tone.wav", "wb")
w.setnchannels(2); w.setsampwidth(2); w.setframerate(48000)
w.writeframes(b"".join(struct.pack("<hh", int(12000*math.sin(2*math.pi*440*t/48000)),
                                           int(12000*math.sin(2*math.pi*660*t/48000)))
                       for t in range(96000)))
w.close()
print("WAV_OK")
PY
aplay -D default /tmp/tone.wav >/dev/null 2>&1; echo "APLAY_RC=$?"
sudo dmesg | grep -i "snd\|audio" | tail -5
cd ~/atl-harness && ./target/release/atlh run --run-id vzs-audio --pkg org.fossify.musicplayer --force 2>&1 | tail -2
echo "== new run audio log:"
grep -i "start_failed\|snd_pcm\|hw_params" work/runs/vzs-audio/org.fossify.musicplayer.log | head -4
echo "== old run (mini/Lima) audio log:"
grep -i "start_failed\|snd_pcm\|hw_params" work/runs/p1w11/org.fossify.musicplayer.log 2>/dev/null | head -4 || ls work/runs | head -20
python3 - <<'PY'
import sqlite3, os
c = sqlite3.connect("work/atlh.sqlite")
for r in c.execute("select run_id,pkg,outcome,interact_ok from apps where run_id in ('vzs-audio') order by run_id"):
    print("DB:", r)
PY
