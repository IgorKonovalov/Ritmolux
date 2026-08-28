Light Music Visualizer - Windows
================================

A music visualizer that listens to whatever your PC is playing and draws to it.

This is an early test build. It is not code-signed, so Windows will warn about
it once. Nothing to install - unzip anywhere and run lmv.exe.


1. Let it run
-------------

Windows SmartScreen will say "Windows protected your PC" because the build is
not signed. Click "More info", then "Run anyway". Only needed the first time.

If you unzipped straight from the browser download and it behaves oddly, right-
click the .zip first, choose Properties, and tick Unblock - then unzip again.


2. Play something
-----------------

Start any music and the visuals react. There is no setup: the app captures
whatever your PC is already playing, through the default output device.

If the visuals move but never react to the beat, they are running on the idle
animation because no audio reached the app. The app says why:

  - Press F3. Under the diagnostics panel there is a line starting with
    "audio". If it starts with "live WASAPI" then sound is reaching the app -
    the rest of the line is the format and the device it is listening to, so
    check that something is playing and that it is playing out of that device.
    If it reads "failed WASAPI ..." then the rest of that line is the reason.
    If it reads "lost WASAPI ..." the input went away mid-run and could not be
    reopened; pick one again from the S menu's Input rows.

  - Or open diagnostics.log (section 4, below). Its last column is named
    "capture" and carries the same sentence on every row.

Either one is worth sending back on its own.


3. Controls
-----------

    Space   next preset
    Tab     browse all presets (arrows to move, Enter to pick, Esc to close)
    S       settings (quality, fullscreen, display)
    F       fullscreen
    D       move to the next monitor
    F3      show frame rate and diagnostics
    A       auto-rotate presets on/off (off by default)

Alt-F4, or close the window, to quit. Escape only closes a menu - it does not
quit the app, so leave fullscreen with F first.

If the frame rate in F3 is poor, press [ to drop to the lighter quality tier
(] raises it again). The app also does this on its own if it cannot hold the
frame budget.


4. Where it keeps its files
---------------------------

    %APPDATA%\light-music-visualizer\

Paste that into the Explorer address bar. It holds an editable copy of the
presets, a config.toml with your settings, and diagnostics.log. Deleting the
folder resets the app; it is recreated on the next launch.

The presets folder next to this file is a reference copy you can read. The app
does not load it - it has its own built in.


5. What to send back
--------------------

Five things, however roughly:

  - Did it run at all, and what did SmartScreen do?
  - Do the visuals react to music? If not, what does F3's "audio" line say?
  - What frame rate does F3 show, and does it say the quality tier was dropped?
  - What graphics card do you have?
  - The contents of diagnostics.log from the folder in step 4.

Thank you - this build exists to find out what breaks.
