<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: Fossify Music plays a track (media3 session gets no commands)

Owner priority. For an interactive opencode session (it can run shell commands);
the overnight loop can't do this one (needs running and debugging).

## Symptom
With the media library (fork main >= 04bbd8ce) Fossify Music lists albums and
tracks. Tapping a track opens the player screen with nothing loaded (00:00,
no title). Log:
```
W/MCImplBase: Controller isn't allowed to call command= 20   (CHANGE_MEDIA_ITEMS)
W/MCImplBase: Controller isn't allowed to call command= 1    (PLAY_PAUSE)
W/MCImplBase: Calling play() omitted due to COMMAND_PLAY_PAUSE not being available
```
So the app's `MediaController` is connected (`isConnected()` is true: Fossify's
SimpleMediaController only calls it then) but its `intersectedPlayerCommands`
are empty = intersection of the session's `availablePlayerCommands` and the
player's `getAvailableCommands()` from the `ConnectionState` it received.

## Reproduce (2 minutes)
```sh
scripts/rig -r mini atlh run --run-id play1 --pkg org.fossify.musicplayer --force
# interaction script interactions/org.fossify.musicplayer.txt: last step fails
scripts/rig -r mini sh 'grep -a MCImplBase work/runs/play1/org.fossify.musicplayer.log'
```

## Already ruled out / fixed (don't redo)
- Package check: media3 rejected its own controller (`getPackagesForUid` was a
  stub) -> fixed in PackageManager.
- `IBinder.isBinderAlive` missing -> added (IBinder, Binder, Messenger, gms stubs).
- Parcel marshalling: not involved. `ATL_BINDER_TRACE=1` shows **zero**
  transactions: controller and session talk in-process via `queryLocalInterface`,
  and the ConnectionState travels as a Java object inside a Bundle
  (`InProcessBinder`). (Parcel binder bugs were fixed anyway.)
- `Binder.equals` treating all binders equal -> fixed (identity); no change.
- Fossify's own code: `MediaSessionCallback.onConnectAsync` accepts with
  defaults; `SimpleMusicPlayer.getAvailableCommands` only removes seek commands.
- `android.util.SparseBooleanArray` (backs media3 `FlagSet`) is AOSP's, looks correct.

## Hypotheses to check next
1. The connection is accepted with EMPTY commands: in media3 `MediaSessionStub`,
   `if (!connectionResult.isAccepted && controllerInfo.isTrusted())` connects
   with `SessionCommands.EMPTY, Player.Commands.EMPTY`. Is the result
   rejected? `isTrusted` comes from androidx `MediaSessionManager`
   (`enabled_notification_listeners`, permission checks: ATL logs
   `Settings$Secure: unknown key enabled_notification_listeners`).
2. `sessionImpl.getPlaybackException() != null` -> restricted command set.
3. ExoPlayer's `getAvailableCommands()` empty: called on the wrong thread, or
   the player wasn't built yet / was released.
4. Two controllers (the app's + media3's media-notification controller); the
   app's controller got the other's result.

## Tools
- `scripts/runner/jdb-app.sh` (on a runner): app under jdb with taps, e.g.
  `scripts/rig -r mini sh 'scripts/runner/jdb-app.sh org.fossify.musicplayer "android.media.AudioTrack.play" "968,1880 300,150" "where"'`
  First attempt with a breakpoint on `android.util.Log.w(String,String)` was
  only "deferred" and never hit: media3's Log wrapper is obfuscated (r8); use a
  class/line from a stack trace in the app log, or `catch` an exception.
- media3 sources (version 1.11.1): github.com/androidx/media, release branch,
  `libraries/session/src/main/java/androidx/media3/session/` (MediaSessionStub,
  MediaControllerImplBase, ConnectionState, MediaSessionServiceStub in
  MediaSessionService.java).
- `ATL_BINDER_TRACE=1` env: logs in-process binder transactions.

## Done when
`interactions/org.fossify.musicplayer.txt` passes on the mini (the player shows
"Alpha Song 1"), no `MCImplBase` warnings, and you hear/see AudioTrack activity.
Then gate (`rig build --ref ...`, `rig atlh run`, `rig atlh compare`). Known
follow-ups once it plays: AudioTrack stop/flush/getTimestamp stubs (triage.md),
AnimationUtils.loadInterpolator returning null.
