Ritmolux - foobar2000 component
=============================================

An audio-reactive visualizer that draws to whatever foobar2000 is playing.
Version @VERSION@.

Unlike the standalone app, this one does not capture your PC's audio - it reads
the samples foobar2000 is already decoding, so there is nothing to permit and
nothing to route.

This is an early build. It is not code-signed.


1. What it needs
----------------

  - foobar2000 v2, 64-bit. Check Help -> About; if it does not say 64-bit,
    this component will not install. There is no 32-bit build.
  - Windows 10 version 1903 or later.
  - A DirectX 12 capable GPU. Integrated graphics are fine - roughly any
    Intel or AMD chip from 2015 on.

Built against the foobar2000 SDK release @SDK_VERSION@.


2. Install it
-------------

  1. In foobar2000: File -> Preferences -> Components.
  2. Click "Install..." and pick foo_ritmolux.fb2k-component (the other file in
     this folder). You can also drag that file onto the Components list.
  3. Click Apply. foobar2000 will ask to restart - let it.

After the restart, "Ritmolux @VERSION@" appears in the Components
list. If the version there does not match this file, an older copy is still
installed - remove it from the same screen first.

To uninstall: the same Components screen, select it, Remove, Apply, restart.


3. Open it
----------

Two ways, and you can use both at once:

  - View -> Ritmolux opens it as its own window.
  - Or dock it into the layout: right-click an empty part of the foobar2000
    window, choose Layout -> Enable layout editing mode, right-click a panel,
    then Replace UI Element (or Split) and pick "Ritmolux" under
    Playback visualisation. Turn layout editing back off when you are done.

Press Space with the visualizer focused to cycle scenes, or right-click it for
the menu (while layout editing is on, a right-click on the docked panel belongs
to foobar2000 instead, so you can still Replace or Remove it):

  - Preset lists every preset that loaded, with a mark on the one showing.
    Pick one and it dissolves across. Your choice is remembered across
    restarts, by name.
  - Reload presets re-reads the folder below, so a file you just dropped in
    shows up without restarting foobar2000.
  - Open presets folder puts you in that folder in Explorer.
  - Next scene is the same step Space takes.

Only one of them renders at a time - whichever you opened last claims the
engine, and the other goes inert until you close it. That is deliberate: one
GPU session, not one per panel.


4. Where it keeps its files
---------------------------

    %APPDATA%\Ritmolux\

Paste that into the Explorer address bar - or use Open presets folder in the
right-click menu, which lands in the presets subfolder directly. It holds an
editable copy of the presets and a diagnostics log. This folder is SHARED with
the standalone app - if you have both, they read the same preset library, and a
preset you edit shows up in both.

To add a preset: drop its .toml into the presets subfolder, then right-click
the visualizer and choose Reload presets. A file the engine cannot parse simply
does not appear in the Preset list.

Deleting the folder resets it; it is recreated on the next launch.


5. If it does not work
----------------------

  - Nothing in the Components list after restarting: you are almost certainly
    on 32-bit foobar2000. See section 1.

  - A DOCKED PANEL is black, or draws but crawls and makes the rest of
    foobar2000 feel frozen: this was a known defect in earlier builds and is
    fixed in this one - the panel now waits for the layout to give it a real
    size before it starts drawing, instead of starting at the wrong one and
    recovering at the first track change. Nothing appears in the Console
    either way, so an empty log is not a clue. If you still see it, that is
    worth telling us: how often it happens is the part we cannot measure from
    here.

  - The POP-OUT window is black and never moves: that one really is the
    engine failing to start. Open View -> Console (foobar2000's own log) and
    look for lines starting with "foo_ritmolux:".

  - It draws, but never reacts to the music: playback has to be running -
    the visuals idle when nothing is playing.

  - You cannot remove the panel while editing the layout: this was a known
    defect too and is fixed in this build - with layout editing on, a
    right-click on the panel now gives you foobar2000's own Cut / Copy /
    Replace / Remove, and our menu comes back when you turn layout editing
    off. If you still get our menu there, Preferences -> Display -> Default
    User Interface removes the panel from the layout tree.


6. What to send back
--------------------

Five things, however roughly:

  - Did it install, and does the Components list show version @VERSION@?
  - Does it react to playback, in the pop-out window and as a docked panel?
  - Did the docked panel come up drawing at a normal speed, before you
    played anything?
  - Does it survive a track change and pressing Space a few times?
  - What graphics card do you have, and any "foo_ritmolux:" lines from the Console.

Thank you - this build exists to find out what breaks.
