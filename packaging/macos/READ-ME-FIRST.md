Ritmolux - macOS
==============================

A music visualizer that listens to whatever your Mac is playing and draws to it.

This is an early test build. It is not signed by Apple, and it has never run on
a Mac before yours - so a couple of the steps below are unusual. They are all
expected. Please read this before double-clicking anything; steps 1 and 2 are
the ones people get stuck on.

You need macOS 13 (Ventura) or newer.


1. Let it open
--------------

macOS blocks apps downloaded from the internet that Apple has not signed. Ours
is not signed, so you have to say so once.

The easy way: right-click (or Control-click) Ritmolux.app, choose
Open, then click Open again in the dialog. Only needed the first time.

The Terminal way, if the dialog will not go away - cd into this folder and run:

    xattr -dr com.apple.quarantine Ritmolux.app

Then open it normally.


2. Grant Screen Recording, then QUIT AND REOPEN IT
--------------------------------------------------

macOS has no way for an app to listen to system audio directly. The only
official route also counts as screen recording, so that is the permission it
asks for. We do not record, keep, or send your screen - the app throws the video
away and keeps the sound.

When the prompt appears, allow it. It should name "Ritmolux".

Then quit the app and open it again. This part is easy to miss: the app cannot
pick the permission up while it is running, so the first launch after granting
is the one that actually hears anything.

If you miss the prompt, it is in System Settings > Privacy & Security > Screen
Recording.


3. If you see visuals but they do not react to the music
--------------------------------------------------------

That is not a crash, and the app is working - it is drawing its idle animation
because no audio reached it. Almost always it means step 2 did not complete:
the permission was denied, or the app was not restarted after granting.

The app says which it was. Either of these answers it, and neither needs the
Terminal:

  - Press F3. Under the diagnostics panel there is a line starting with
    "audio". If it starts with "live SCK" then sound is reaching the app and
    the problem is elsewhere - check that something is actually playing. If it
    reads "failed SCK ..." then the rest of that line is the reason. A photo of
    the screen is enough.

  - Or open diagnostics.log (step 5, below). Its last column is named "capture"
    and carries the same sentence on every row.

Send back whichever you get - that line is the whole answer.

If neither is available for some reason, the fallback is to run the app from
Terminal, which prints the same reason on startup - cd into this folder and run:

    ./Ritmolux.app/Contents/MacOS/ritmolux


4. Controls
-----------

    Space   next preset
    Tab     browse all presets (arrows to move, Enter to pick, Esc to close)
    S       settings (quality, fullscreen, display)
    F       fullscreen
    F3      show frame rate and diagnostics
    A       auto-rotate presets on/off (off by default)

Command-Q, or close the window, to quit. (Escape only closes a menu - it does
not quit the app, and in fullscreen there is no window button, so use F to leave
fullscreen first or press Command-Q.)


5. Where it keeps its files
---------------------------

    ~/Library/Application Support/ritmolux/

That folder has an editable copy of the presets, a config.toml with your
settings, and diagnostics.log. Deleting the folder resets the app; it will be
recreated on the next launch.

The presets folder next to this file is a reference copy you can read. The app
does not load it - it has its own built in.


6. What to send back
--------------------

Six things, however roughly:

  - Did the app open at all?
  - Did the permission prompt say "Ritmolux"?
  - After granting and reopening, do the visuals react to music?
  - What frame rate does F3 show, and what does its "audio" line say?
  - The contents of the diagnostics.log file from the folder in step 5.
  - If anything went wrong, whatever step 3 told you the reason was.

Thank you - this build exists to find out what breaks.


A note on updates
-----------------

Each new build we send you is a different app as far as macOS is concerned, so
you will have to grant Screen Recording again every time, and old entries pile
up in that Privacy list. Annoying, and known. Removing the old entries is safe.
