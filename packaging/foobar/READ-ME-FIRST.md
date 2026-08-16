Light Music Visualizer - foobar2000 component
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
  2. Click "Install..." and pick foo_lmv.fb2k-component (the other file in
     this folder). You can also drag that file onto the Components list.
  3. Click Apply. foobar2000 will ask to restart - let it.

After the restart, "Light Music Visualizer @VERSION@" appears in the Components
list. If the version there does not match this file, an older copy is still
installed - remove it from the same screen first.

To uninstall: the same Components screen, select it, Remove, Apply, restart.


3. Open it
----------

Two ways, and you can use both at once:

  - View -> Light Music Visualizer opens it as its own window.
  - Or dock it into the layout: right-click an empty part of the foobar2000
    window, choose Layout -> Enable layout editing mode, right-click a panel,
    then Replace UI Element (or Split) and pick "Light Music Visualizer" under
    Playback visualisation. Turn layout editing back off when you are done.

Press Space with the visualizer focused to cycle scenes.

Only one of them renders at a time - whichever you opened last claims the
engine, and the other goes inert until you close it. That is deliberate: one
GPU session, not one per panel.


4. Where it keeps its files
---------------------------

    %APPDATA%\light-music-visualizer\

Paste that into the Explorer address bar. It holds an editable copy of the
presets and a diagnostics log. This folder is SHARED with the standalone app -
if you have both, they read the same preset library, and a preset you edit
shows up in both.

Deleting the folder resets it; it is recreated on the next launch.


5. If it does not work
----------------------

  - Nothing in the Components list after restarting: you are almost certainly
    on 32-bit foobar2000. See section 1.

  - A DOCKED PANEL is black and never moves: this one is a known defect in
    this build, not a broken install. Play a track and let it change to the
    next one - the panel usually comes to life at a track boundary and stays
    alive from then on. Nothing appears in the Console when this happens, so
    an empty log is not a clue. Please tell us if you hit it; how often it
    happens is the part we cannot measure from here.

  - The POP-OUT window is black and never moves: that one really is the
    engine failing to start. Open View -> Console (foobar2000's own log) and
    look for lines starting with "foo_lmv:".

  - It draws, but never reacts to the music: playback has to be running -
    the visuals idle when nothing is playing.

  - You cannot remove the panel while editing the layout: right-click gives
    you our menu instead of foobar2000's, which is a known defect too. Use
    Preferences -> Display -> Default User Interface and remove it from the
    layout tree there.


6. What to send back
--------------------

Five things, however roughly:

  - Did it install, and does the Components list show version @VERSION@?
  - Does it react to playback, in the pop-out window and as a docked panel?
  - Was the docked panel black at first, and did a track change fix it?
  - Does it survive a track change and pressing Space a few times?
  - What graphics card do you have, and any "foo_lmv:" lines from the Console.

Thank you - this build exists to find out what breaks.
