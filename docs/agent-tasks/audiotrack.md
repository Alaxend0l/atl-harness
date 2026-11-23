<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: AudioTrack (ALSA) with real play/pause/stop/flush, positions and timestamps

For an interactive opencode session in a worktree of the ATL fork
(`~/Develop/android-translation-layer`). Read AGENTS.md ("ATL fork gotchas")
first. Paths below are relative to the fork root; line numbers are from
fork `main` at `9a2cd6c4`.

## Symptom / why it matters
Fossify Music (`org.fossify.musicplayer`) plays through androidx.media3 1.11.1
ExoPlayer, and every game/app that streams PCM uses `android.media.AudioTrack`.
media3 drives the track like this (sources: github.com/androidx/media tag
`1.11.1`, `libraries/exoplayer/src/main/java/androidx/media3/exoplayer/audio/`):
- `AudioTrackAudioOutput.write` (l. 261) calls
  `audioTrack.write(buffer, n, AudioTrack.WRITE_NON_BLOCKING)` from the playback
  thread, **also while paused** (ExoPlayer keeps rendering at a low rate) and
  before `play()` (pre-roll). It expects 0 when the track is full, never a block.
- `play()`/`pause()` (l. 224-237) call `audioTrack.play()`/`pause()`.
- Seek: `DefaultAudioSink.flush()` (l. 1588-1601) releases the output:
  `pause()` now, then on a background thread `flush()` + `release()`
  (l. 485-499), and opens a new AudioTrack immediately.
- Track end: `stop()` (l. 298-306) calls `handleEndOfStream(writtenFrames)` then
  `audioTrack.stop()`; the position after that is simulated from the wall clock
  (`AudioTrackPositionTracker` l. 511-522), so stop must let queued audio play out.
- Position: `AudioTrackPositionTracker.updateRawPlaybackHeadPosition`
  (l. 524-560) reads `getPlaybackHeadPosition()` every 5 ms while not STOPPED.
  **If the value ever decreases it assumes a 32-bit wrap** (l. 556) and adds
  2^32 frames: position jumps to the end, the track "ends". So the head
  position must be monotonic between flushes.
- `getTimestamp()` is polled by `AudioTimestampPoller`; `false` is allowed
  (falls back to head-position smoothing after 500 ms), a wrong `true` is not.

What breaks today: blocking ALSA writes on a paused/full PCM hang the
ExoPlayer thread (pause -> resume/seek do nothing), audio auto-starts before
`play()` once the buffer fills, `stop()`/`flush()` do nothing, the head
position can go backwards (uninitialised `delay`) and is never reset.

## Current code (what is wrong)
`src/api-impl/android/media/AudioTrack.java`
- l. 27-38: state is a Java int; `playbackHeadPosition` (frames written, `int`)
  is never reset; `frames` is a static shared by all tracks.
- l. 77-81 `play()`: sets PLAYING, `native_play()`.
- l. 83-86 `stop()`: **stub**, only sets STOPPED. l. 88-90 `flush()`: **stub**.
- l. 92-95 `release()`: `native_release()`; handle stays non-zero (use-after-free
  if called again or followed by any other call).
- l. 97-99 `getState()`: always 1 even when ALSA open/hw_params failed.
- l. 101-116 `write(byte[])`: assumes PCM16 (`/ channels / 2`); always blocking.
- l. 118-122 `write(ByteBuffer,int,int)`: ignores `writeMode`; uses
  `audioData.array()` (throws for buffers without a backing array).
- l. 124-139 `write(short[])`: same, always blocking.
- l. 197-199 `getBufferSizeInFrames()`: requested size, not ALSA's real buffer.
- l. 206-210 `pause()`: PAUSED + `native_pause()`.
- l. 212-214 `getPlaybackHeadPosition()`: `written - native delay`; not
  monotonic, not 0 when STOPPED/flushed.
- l. 221-223 `getTimestamp()`: always `false`. No `WRITE_BLOCKING`/
  `WRITE_NON_BLOCKING`/`STATE_*` constants.
`src/api-impl/android/media/AudioTimestamp.java`: **empty class** (no
`framePosition`/`nanoTime` fields: media3 would hit NoSuchFieldError).
`src/api-impl-jni/audio/android_media_AudioTrack.c`
- l. 13-94 `native_constructor`: errors only printed (l. 27-29, 45-47);
  l. 63 start threshold = whole buffer, so ALSA **auto-starts** when the buffer
  fills, even if `play()` was never called; PCM is in blocking mode.
- l. 167-195 `native_play`: adds a new SIGIO `snd_async_add_pcm_handler` on
  **every** play (leaks, duplicates callbacks), then `snd_pcm_start` +
  `snd_pcm_pause(FALSE)` regardless of state.
- l. 197-215 `write_frames`: volume loop uses `frames * 2` (assumes stereo:
  overruns mono buffers) and scales **the app's own array in place**
  (released with mode 0 at l. 223/234, copying it back); `snd_pcm_writei`
  blocks; on error returns 0 (track looks "full" forever).
- l. 239-243 `native_pause`: return value of `snd_pcm_pause` ignored.
- l. 245-249 `native_release`: closes, does not clear the Java field.
- l. 251-258 `native_getPlaybackHeadPosition`: ignores the `snd_pcm_delay`
  return; on XRUN (-EPIPE) `delay` is uninitialised garbage.
`src/api-impl-jni/audio/audio.c` l. 9-18: channel mask 3
(`CHANNEL_CONFIGURATION_STEREO`) maps to mono. ATL's MediaCodec outputs S16
(`src/api-impl-jni/media/android_media_MediaCodec.c` l. 367), so PCM16 is the
format that matters; media3 only uses float if the app enables float output.

## What Android does (AOSP frameworks/base `media/java/android/media/AudioTrack.java`, frameworks/av `media/libaudioclient/AudioTrack.cpp`)
- Constants: `STATE_UNINITIALIZED=0`, `STATE_INITIALIZED=1`; `PLAYSTATE_STOPPED=1`,
  `PAUSED=2`, `PLAYING=3`; `WRITE_BLOCKING=0`, `WRITE_NON_BLOCKING=1`;
  `SUCCESS=0`, `ERROR=-1`, `ERROR_BAD_VALUE=-2`, `ERROR_INVALID_OPERATION=-3`,
  `ERROR_DEAD_OBJECT=-6`. A new track is `PLAYSTATE_STOPPED`.
- Data written before `play()` is queued and does **not** play until `play()`.
- `pause()`: "Data that has not been played back will not be discarded.
  Subsequent calls to play will play this data back."
- `stop()`: "When used on an instance created in MODE_STREAM mode, audio will
  stop playing after the last buffer that was written has been played. For an
  immediate stop, use pause(), followed by flush()". Returns immediately;
  play state becomes STOPPED at once (native: no-op unless ACTIVE or PAUSED).
- `flush()`: "Any data that has been written but not yet presented will be
  discarded. No-op if not stopped or paused" (native `flush()` returns if
  `mState == STATE_ACTIVE`). Resets the position.
- `getPlaybackHeadPosition()`: frames presented since start, unsigned 32-bit
  wrapping. `AudioTrack::getPosition`: "IAudioTrack::stop() isn't synchronous;
  we don't know when presentation completes": returns **0 when STOPPED or
  FLUSHED**. `start()` from STOPPED/FLUSHED: "reset current position as seen by
  client to 0". From PAUSED it continues.
- `write(..., WRITE_BLOCKING)` blocks until all data is queued, but "if the track
  is stopped or paused on entry, or another thread interrupts the write by
  calling stop or pause ... the write may return a short transfer count".
  `WRITE_NON_BLOCKING`: "return immediately after queuing as much audio data
  for playback as possible without blocking" (may be 0). Return = bytes (or
  shorts/floats for those overloads), a whole number of frames.
  `write(ByteBuffer)` advances `position()` by the bytes written.
- `getTimestamp(AudioTimestamp ts)`: `IllegalArgumentException` if null. Returns
  true and fills `ts.framePosition` (frames) and `ts.nanoTime` (CLOCK_MONOTONIC,
  same base as `System.nanoTime()`) = "the estimated time when that frame was
  presented or is committed to be presented"; returns false (instance left
  unaltered) when unavailable: STOPPED/FLUSHED or just after start before
  frames reached the output.
- `getState()`: `STATE_UNINITIALIZED` if the native track could not be created;
  then `play()/pause()/stop()` throw `IllegalStateException`.

## Implementation steps (one per commit-sized change; build after each)
Build: `scripts/rig build --worktree <worktree>` (reports parsed errors).

0. **Harness (this repo, not the fork): add `expect-same [PCT]`** to
   `src/interact.rs`: `Step::ExpectSame(f64)`, parsed like `expect-changed`
   (l. 124-125, default `DEFAULT_CHANGED`), executed like l. 264-276 but
   failing when `f > max` ("screen changed (x% of pixels differ, allow <= y%)").
   Add it to the module doc (l. 12-15), the parser unit test (~l. 330-346),
   and docs/usage.md "Interaction scripts". `cargo fmt && cargo clippy
   --all-targets && cargo test`. Then write the failing script (next section).
1. **Java constants/fields** in `AudioTrack.java`: add `STATE_UNINITIALIZED=0`,
   `STATE_INITIALIZED=1`, `WRITE_BLOCKING=0`, `WRITE_NON_BLOCKING=1`,
   `SUCCESS=0`, `ERROR=-1`, `ERROR_INVALID_OPERATION=-3`, `ERROR_DEAD_OBJECT=-6`.
   Replace `playbackHeadPosition` with `long framesWritten`, `long positionBase`
   (frames played before the last reset), `long lastPosition` (monotonic guard),
   `int frameSize` (bytes per frame), `int buffer_frames`, `int period_frames`
   (set by native). Leave the static `frames` field as it is.
   `AudioTimestamp.java`: add `public long framePosition; public long nanoTime;`
   and `TIMEBASE_MONOTONIC = 0`, `TIMEBASE_BOOTTIME = 1`.
2. **Constructor (C, l. 13-94)**: check every ALSA return; on failure of
   `snd_pcm_open` or `snd_pcm_hw_params` close the handle (if open) and set
   `pcm_handle` to 0, then return. After `snd_pcm_hw_params`: read
   `snd_pcm_hw_params_get_buffer_size` and `_get_period_size` into the Java
   fields `buffer_frames`/`period_frames`. sw params: replace l. 63 with
   `snd_pcm_sw_params_get_boundary(sw_params, &boundary);
   snd_pcm_sw_params_set_start_threshold(pcm, sw_params, boundary);` (never
   auto-start; we start explicitly). Keep avail_min = period. Free sw_params
   with `snd_pcm_sw_params_free`. Finally `snd_pcm_nonblock(pcm, 1)` (all
   blocking is done by us with `snd_pcm_wait`). Java: `getState()` returns
   `pcm_handle != 0 ? STATE_INITIALIZED : STATE_UNINITIALIZED`;
   `getBufferSizeInFrames()` returns `buffer_frames`. frameSize =
   channels * 2 for now (step 9 adds float).
3. **Native write** (C l. 197-237). New Java natives (overloads stay):
   `private native int native_write(byte[] d, int offsetInBytes, int frames, float volume, boolean blocking, boolean playing);`
   `private native int native_write(short[] d, int offsetInShorts, int frames, float volume, boolean blocking, boolean playing);`
   JNI names `Java_android_media_AudioTrack_native_1write___3BIIFZZ` and
   `..._native_1write___3SIIFZZ`, signatures `([BIIFZZ)I` / `([SIIFZZ)I`.
   **Update `src/api-impl-jni/generated_headers/android_media_AudioTrack.h` by
   hand** (copy an existing entry's format; worktree builds may not regenerate
   it). Logic of `write_frames(pcm, ptr, frames, channels, volume, blocking, playing)`:
   - if `volume != 1`: `malloc` a copy, scale `frames * channels` samples of the
     copy (clamp to int16), write the copy, free it. Never modify the Java
     array; release arrays with `JNI_ABORT` (don't use `_RELEASE_BYTE_ARRAY_ELEMENTS`,
     it passes mode 0).
   - loop while `done < frames`: `n = snd_pcm_writei(pcm, ptr + done*frame_bytes, frames - done)`.
     `n > 0`: `done += n`. `n == -EAGAIN`: if `!blocking || !playing` break; else
     `snd_pcm_wait(pcm, 100)` and continue. `n == -EPIPE`: `snd_pcm_prepare(pcm)`
     and continue. `n == -ESTRPIPE`: `snd_pcm_resume`, if that fails
     `snd_pcm_prepare`; continue. Other error: break, and if `done == 0`
     return -1 (Java returns `ERROR`).
   - after the loop: if `playing && snd_pcm_state(pcm) == SND_PCM_STATE_PREPARED
     && done > 0` -> `snd_pcm_start(pcm)`.
   Java side: `write(byte[],..)` = blocking; `write(short[],..)` = blocking;
   `write(ByteBuffer, size, mode)`: return `ERROR_BAD_VALUE` for a bad mode or
   `size > remaining()`; if `hasArray()` use `array()/arrayOffset()`, else copy
   `size` bytes into a temp `byte[]` with `duplicate().get(...)`; pass
   `mode == WRITE_BLOCKING`. Add `write(ByteBuffer, int, int, long)` only if
   something calls it (tunneling; not needed for Fossify). All writes:
   `framesWritten += framesDone`; return `framesDone * frameSize` (bytes) or
   `framesDone * channels` (shorts); advance the ByteBuffer by the bytes.
   Return `ERROR_INVALID_OPERATION` when `pcm_handle == 0`.
4. **Delay / head position**. Replace `native_getPlaybackHeadPosition` with
   `private native int native_getDelay();` (`()I`, rename in the header too):
   `snd_pcm_state` RUNNING/PAUSED/PREPARED/DRAINING -> `snd_pcm_delay`; if it
   returns <0 or the state is XRUN/SETUP -> return 0 (all written frames have
   been played); clamp to `[0, buffer_frames]`. Java:
   ```java
   public int getPlaybackHeadPosition() {
       if (pcm_handle == 0 || playbackState == PLAYSTATE_STOPPED) return 0; // AOSP
       long played = framesWritten - native_getDelay() - positionBase;
       if (played < lastPosition) played = lastPosition; // media3 treats a decrease as a wrap
       lastPosition = played;
       return (int) played;
   }
   ```
5. **play / pause** (C l. 167-195, 239-243). `native_play`: register the async
   SIGIO handler **only if** `periodic_update_listener != null` and not already
   registered (store the handler pointer in a new `long async_handler` field);
   then switch on `snd_pcm_state`: PAUSED -> `snd_pcm_pause(pcm, 0)`; PREPARED ->
   `snd_pcm_start` only if `buffer_frames - snd_pcm_avail(pcm) > 0`, else nothing
   (the next write starts it); XRUN -> `snd_pcm_prepare` (next write starts);
   RUNNING -> nothing. `native_pause`: only if RUNNING: `snd_pcm_pause(pcm, 1)`;
   if that fails (device can't pause, check `snd_pcm_hw_params_can_pause`) log
   once and fall back to `snd_pcm_drop` + `snd_pcm_prepare` (queued audio is
   lost; position stays monotonic because of the guard). Java `play()`:
   throw `IllegalStateException` if uninitialized; if the previous state was
   STOPPED: `positionBase = framesWritten - native_getDelay(); lastPosition = 0;`
   (AOSP resets the position on start from STOPPED/FLUSHED); then
   `native_play()`; state = PLAYING. `pause()`: `native_pause()`; state = PAUSED.
6. **stop** (new `public native void native_stop();`, `()V`, header): PREPARED
   with queued frames -> `snd_pcm_start` (short tracks that never started must
   still play out); RUNNING -> leave it running: it plays the rest and then
   underruns into XRUN, which is the drain. **Do not call `snd_pcm_drain`**: it
   blocks the caller (and is unreliable in non-blocking mode on the PipeWire/
   Pulse ioplug devices). PAUSED -> `snd_pcm_drop` + `snd_pcm_prepare`.
   Java `stop()`: throw `IllegalStateException` if uninitialized;
   `native_stop()`; state = STOPPED. Remove the "STUB" print.
7. **flush** (new `public native void native_flush();`, `()V`, header):
   `snd_pcm_drop(pcm); snd_pcm_prepare(pcm);`. Java `flush()`: if
   uninitialized or `playbackState == PLAYSTATE_PLAYING` return (AOSP no-op);
   else `native_flush(); framesWritten = 0; positionBase = 0; lastPosition = 0;`.
   **release**: `native_release` closes and sets `pcm_handle` to 0 (and frees
   the async handler); Java `release()` returns early if already 0 and sets
   state STOPPED. Every native must return early when the handle is 0.
8. **getTimestamp** (new `private native boolean native_getTimestamp(long[] out);`,
   `([J)Z`, header): only if `snd_pcm_state == RUNNING`: `snd_pcm_delay(pcm, &d)`
   (fail -> false), then `clock_gettime(CLOCK_MONOTONIC, &ts)`; `out[0] = d`,
   `out[1] = ts.tv_sec * 1000000000LL + ts.tv_nsec` (via `SetLongArrayRegion`).
   Java: null -> `IllegalArgumentException`; return false unless PLAYING and
   initialized; `pos = framesWritten - out[0] - positionBase`; false if `pos <= 0`;
   else `ts.framePosition = pos; ts.nanoTime = out[1]; return true`. (Better
   later: `snd_pcm_status` + `snd_pcm_status_get_htstamp` with sw_params
   `tstamp_type = SND_PCM_TSTAMP_TYPE_MONOTONIC`; the default type is
   gettimeofday, which is NOT `System.nanoTime()`'s clock.)
9. (Optional) **Encodings**: `frameSize` from the encoding: 1/2 (DEFAULT/PCM_16BIT)
   -> S16_LE, 2 bytes; 3 (PCM_8BIT) -> `SND_PCM_FORMAT_U8`, 1; 4 (PCM_FLOAT) ->
   `SND_PCM_FORMAT_FLOAT_LE`, 4 (plus `write(float[], int, int, int)`); anything
   else (AC3, ...) -> `STATE_UNINITIALIZED`. Pass the format to
   `helper_hw_params_init` and use the same size in `getMinBufferSize`. In
   `audio.c` map mask 3 to 2 channels.

## Failing check to write first (docs/agent-tasks/test-first.md)
Extend `interactions/org.fossify.musicplayer.txt` (needs step 0). Coordinates
from the 1080x1920 player screenshot (`work/remote/mini/runs/play8/
org.fossify.musicplayer.playing.png`): play/pause 540 636; seek bar y=476 from
x=25 to x=1058; elapsed time at 40 514; Alpha Song 1 is 21 s long and the
player shows "Next: Alpha Song 2". Append after the last line:
```
wait 2
shot play1
wait 3
shot play2
expect-changed             # elapsed time / seek thumb advance while playing
tap 540 636                # pause
wait 2
shot pause1
wait 3
shot pause2
expect-same                # position frozen while paused
tap 540 636                # resume
wait 1
shot resume1
wait 3
shot resume2
expect-changed             # resume works (hangs today if a blocked write wedged the player thread)
tap 130 476                # seek to ~10 % (00:02)
wait 2
shot seek
expect-text "00:0"         # was ~00:10 before the seek
tap 1000 476               # seek to ~95 %, let the track end (stop/end of stream)
wait 7
shot ended
expect-text "Alpha Song 3" # auto-advanced: "Next: Alpha Song 3"
```
Run it on the unfixed fork **twice** on a runner with **working audio**; it must
fail the same way both times. Note which step fails and put it in the commit
message. Prerequisite: mini's ALSA device fails `snd_pcm_hw_params` (no audio
server: logs show `ERROR: Can't set harware parameters`), so its position never
moves and this test is meaningless there. Check first:
`scripts/rig -r bazzite sh 'aplay -l; timeout 3 speaker-test -D default -c 2 -l 1'`
and grep the run log for `Can't set harware parameters`. If no runner has
working audio, stop and report (don't paper over it). If tapping the seek bar
doesn't seek in ATL (a SeekBar bug, separate task), drop the two seek blocks
and say so. If the whole script already passes on the unfixed fork, add a
log check instead: no `STUB: AudioTrack` lines and no `XRUN.` storms.

## Done when
- `scripts/rig build --worktree <worktree>` is clean.
- The extended script passes on the audio-capable runner, twice.
- `rig atlh run` (Fossify set + corpus) and `rig atlh compare <base> <new>`:
  0 regressions; the run log has no `STUB: AudioTrack` lines.
- Unit/format checks for the harness change (step 0) pass.
- HANDOFF.md / triage.md updated (stop/flush/getTimestamp items resolved).

## Risks / what not to do
- No fake fixes (AGENTS.md): no `catch (Throwable)`, no returning constant
  positions, no "pretend played" clocks when ALSA fails. A device that can't
  open is `STATE_UNINITIALIZED` (media3 then throws InitializationException,
  which is the honest result). Check the existing title steps of the script
  still pass on mini after step 2; if they don't, report it rather than hiding it.
- Never let the head position go backwards except via flush/stop+play (media3
  wraps it to +2^32 frames: the track jumps to its end).
- Never block in `write(..., WRITE_NON_BLOCKING)`, `pause()`, `stop()` or
  `flush()`. No `snd_pcm_drain`. Blocking writes must give up when not PLAYING.
- `snd_pcm_delay` on XRUN returns -EPIPE without setting the output: always
  initialise it and check the return.
- Do not call JNI from the SIGIO callback more than before; don't add new
  async handlers. No Java lambdas in `src/api-impl` (dx).
- Release happens on media3's "AudioTrackReleaseThread" while a new track is
  opened on the player thread: no shared static state between tracks.
- Don't touch upstream ATL (AGENTS.md hard rule).

## Also found on the phone (2026-11-21, add as steps after the above)
- **Segfault when the device can't be opened.** If `snd_pcm_open` fails
  ("Unknown PCM default", no ALSA config/plugin), ATL keeps going with a NULL
  handle and ExoPlayer's playback thread dies with SIGSEGV. Android's
  behaviour: the AudioTrack ends up `STATE_UNINITIALIZED` (getState() 0),
  `play()` throws `IllegalStateException`, `write()` returns
  `ERROR_INVALID_OPERATION` (-3); media3 then reports a playback error instead
  of crashing. Check every use of the PCM handle for NULL.
- **Phone audio path.** PipeWire owns the sound card. ATL's ALSA output must
  go through the ALSA→PipeWire plugin (`pipewire-alsa` in the rootfs,
  scripts/runner/packages.txt; ALSA's `default` PCM then is PipeWire). Don't
  open `hw:` devices directly.
- **Equalizer is a stub** (`src/api-impl/android/media/audiofx/Equalizer.java`:
  empty constructor and `usePreset`): Fossify Music's
  `SimpleEqualizer.setupEqualizer` reads `enabled`, `bandLevelRange`,
  `getBandLevel`, presets → exception → toast "An unknown error occurred" on
  every playback start. Implement the AOSP Equalizer API (`getNumberOfBands`,
  `getBandLevelRange` (-1500..1500 mB), `getCenterFreq`, `getBandFreqRange`,
  `getBand`, `get/setBandLevel`, `getNumberOfPresets`, `getPresetName`,
  `getCurrentPreset`, `usePreset`, `get/setEnabled` from AudioEffect,
  `getProperties/setProperties`) with 5 bands (60, 230, 910, 3600, 14000 Hz)
  and the usual presets (Normal, Classical, Dance, Flat, Folk, Heavy Metal,
  Hip Hop, Jazz, Pop, Rock) and, as a second step, actually apply it: one
  peaking biquad per band (RBJ cookbook) on the PCM in `AudioTrack.write` for
  tracks whose session id has an enabled Equalizer. Until the filtering
  exists, report `hasControl()` honestly and keep levels as state.

## Status update (2026-11-23, `fix/fixcov-crashes` @ `a52665e6`, runner oneplus12)
`getPlaybackHeadPosition` is now monotonic (last reported value is the
floor; counters reset in `flush()` only, per AOSP) — commit `e20d7f16`.
This **fixed the owner-reported Gramophone bug** ("pause then resume does
not take effect"): the PipeWire-backed delay can exceed frames written
across `snd_pcm_pause`, the old clamp-to-0 rewound the head ~8 s at the
moment of pause, media3's `AudioTrackPositionTracker` logged a phantom
`audioTrackUnderrun` and never re-primed the track on resume (ExoPlayer
stuck BUFFERING, source Loader stuck in `pauseLoading`, zero
`AudioTrack.play()` calls — witnessed with temporary
`Thread.getAllStackTraces` probes in `MediaSession.setPlaybackState`).
Witness after the fix (runs `grres22..grres24`): two media-key
pause→resume cycles, `AudioTrack.play()` on every resume, position
8158→15581 ms, no underrun. Note the Fossify line numbers above are
media3 1.11; Gramophone 1.1.2 uses nift4's media3 fork
(`github.com/nift4/media`, branch `gramophone`, submodule `962f72c3`) and
its vendored `androidx.media3.session.legacy.PlaybackStateCompat`
**renumbers playback states** (BUFFERING=6, ERROR=7, CONNECTING=8), which
matters for anything reading published session states (the media-key
expansion does: `ATL_STATE_BUFFERING_LEGACY`).
Still open from this brief: `snd_pcm_delay()`'s error return is still
ignored in the JNI (the monotonic clamp covers the symptom);
`getTimestamp()` still returns false; the Equalizer stub and the
NULL-handle steps above are not done.
