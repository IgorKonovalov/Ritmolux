Ritmolux - Linux
==============================

A music visualizer that listens to whatever your computer is playing and draws
to it.

This is an early test build for Ubuntu 24.04 or newer on x86_64. Nothing to
install - unpack it anywhere and run ritmolux.


1. Let it run
-------------

From a terminal in the unpacked folder:

    chmod +x ritmolux
    ./ritmolux

The chmod is only needed if the file lost its executable bit on the way to you;
running it again is harmless. Start it from your desktop session, not over SSH -
it needs your display and your sound server.

It needs PipeWire or PulseAudio, which every Ubuntu desktop already runs. If it
stops at once with "error while loading shared libraries: libpulse.so.0", the
machine has neither; on Ubuntu, `sudo apt install libpulse0` fixes it.


2. Play something
-----------------

Start any music and the visuals react. There is no setup: the app captures
whatever your computer is already playing, through the default output device -
the one your system sound settings show as selected.

If the visuals move but never react to the beat, they are running on the idle
animation because no audio reached the app. The app says why:

  - Press F3. Under the diagnostics panel there is a line starting with
    "audio". If it reads "live PulseAudio 48000/2 @DEFAULT_MONITOR@" then sound
    is reaching the app - check that something is playing, and that it is
    playing out of the output your sound settings have selected. If it reads
    "failed PulseAudio ..." then the rest of that line is the reason. If it
    reads "lost PulseAudio ..." the sound server went away mid-run and could
    not be reopened; restart the app.

  - Or open diagnostics.log (section 4, below). Its last column is named
    "capture" and carries the same sentence on every row.

Either one is worth sending back on its own.

The app always listens to the default output. To have it hear a different one,
change the default in your system sound settings - there is no device picker on
Linux yet.


3. Controls
-----------

    Space   next preset
    Tab     browse all presets (arrows to move, Enter to pick, Esc to close)
    S       settings (quality, fullscreen, display)
    F       fullscreen
    D       move to the next monitor
    F3      show frame rate and diagnostics
    A       auto-rotate presets on/off (off by default)

Close the window to quit. Escape only closes a menu - it does not quit the app,
so leave fullscreen with F first.

Under a Wayland session (the Ubuntu default) the compositor decides where
windows go, so D may do nothing. Use your desktop's own move-to-monitor
shortcut instead.

If the frame rate in F3 is poor, press [ to drop to the lighter quality tier
(] raises it again). The app also does this on its own if it cannot hold the
frame budget.


4. Where it keeps its files
---------------------------

    ~/.local/share/Ritmolux/

(or $XDG_DATA_HOME/Ritmolux/ if you have set XDG_DATA_HOME). The capital R
matters. It holds an editable copy of the presets, a config.toml with your
settings, and diagnostics.log. Deleting the folder resets the app; it is
recreated on the next launch.

The presets folder next to this file is a reference copy you can read. The app
does not load it - it has its own built in.


5. What to send back
--------------------

Five things, however roughly:

  - Which distribution and version, and is the session Wayland or X11?
    (`echo $XDG_SESSION_TYPE` answers the second.)
  - Do the visuals react to music? If not, what does F3's "audio" line say?
  - What frame rate does F3 show, and does it say the quality tier was dropped?
  - What graphics card do you have?
  - The contents of diagnostics.log from the folder in step 4.

Thank you - this build exists to find out what breaks.
