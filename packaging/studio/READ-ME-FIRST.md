Ritmolux Studio - Windows and macOS
===================================

Ritmolux draws to whatever your computer is playing. The Studio is the window
where you change what it draws, while it is drawing it.

Everything is in this folder. The player is inside the Studio - there is no
second download and nothing to point it at.

This is an early test build. It is not code-signed, so your computer will warn
about it once.


1. Let it run
-------------

WINDOWS

Run "Ritmolux Studio.exe". SmartScreen will say "Windows protected your PC"
because the build is not signed. Click "More info", then "Run anyway". Only
needed the first time.

If you unzipped straight from the browser download and it behaves oddly, right-
click the .zip first, choose Properties, and tick Unblock - then unzip again.

MACOS

Right-click "Ritmolux Studio.app" and choose Open, then Open again in the
dialog. Double-clicking will not work the first time: the app is only ad-hoc
signed, so macOS quarantines it.

The first time it starts a player, macOS will ask for Screen Recording. That
permission is how an app hears the system's sound on macOS - grant it and
restart the Studio, or you will get a picture that never reacts.

Every new build has a different signature, so macOS treats it as a different app
and asks again. That is expected on an unsigned build.


2. What you should see
----------------------

TWO windows. The Studio, and the player's own full picture beside it. They are
the same show: the Studio paints a copy of the player's frames, so a change you
make appears in both.

ON ONE SCREEN, that second window is in the way. Click "settings" in the top
right of the Studio and choose "windowless". Close the Studio and open it again;
this time the player opens no window of its own and the Studio's own picture is
the whole of it. The Studio remembers the choice.

That mode is for editing on a laptop. When you are actually showing something to
people, put it back on "windowed" - only then is the picture in the Studio the
same picture the room is looking at.

The Studio's picture is a small, scaled copy either way, so it is not the place
to judge a fine detail - the player's own window is.

Start any music. The picture reacts to whatever is already playing, out of the
device your computer is playing it through. Nothing needs setting up first.

IF THE PICTURE DOES NOT REACT to music you can plainly hear, the player is
listening to the wrong thing - which does happen, and is worth reporting rather
than working around. Open diagnostics.log in the folder named in section 4 and
look at the last column of any row. It names the device the player opened:

    live WASAPI 48000/2 Speakers (Realtek(R) Audio)

If that names a microphone or a headset input instead of the speakers or
headphones you are actually listening to, that is the fault, and that line is
the single most useful thing to send back. Setting your default playback device
in the system sound settings and starting the Studio again is worth one try; if
it changes nothing, say so - that is a finding, not something you did wrong.

Along the bottom of the Studio is a strip of readings: the player's version, the
address it is listening on, the size and rate of the picture, and how many
frames the Studio painted and dropped. Dropped frames there mean the Studio's
preview is behind - the player's own window is unaffected.

IF THE STUDIO SAYS "No player found", something is wrong with this zip rather
than with your machine; that is worth sending back exactly as it appears.


3. Changing what it draws
-------------------------

The right-hand panel has five tabs.

    parameters   Every number the current look declares, with a slider or a
                 field. Drag one and the picture moves as you drag. Let go
                 and the value is saved.

    structure    The tables underneath a look - how many particles, which
                 generator, how the layers stack.

    palette      The colours, as stops you can drag.

    file         The preset as text. This is where expressions live - the small
                 language that ties a value to the music. Save with Ctrl-S
                 (Cmd-S on macOS).

    library      Every preset the player has. Click one to switch to it.

Parameters are live; the other four go through the file on disk, so the picture
follows about a fifth of a second later, when the player notices the change.

THE FIRST CHANGE YOU MAKE TO A PRESET MAKES A COPY. The Studio never writes a
preset it did not create. It asks what to call the copy, saves the whole thing
under that name, and switches to it; everything you do afterwards goes to your
copy without asking again. The preset you started from is left exactly as it
was, and your copy sits beside it in the library tab.

ONE EXCEPTION, and it is worth knowing before it surprises you. A few looks are
built into the player rather than kept as files, and the Studio cannot read one
of those. Copying it gives you the plain starting point for that kind of look
plus the change you just made - not the look you were watching. If your copy
comes out looking nothing like what was on screen, that is why, and it is
expected rather than a fault.

Closing the Studio forgets which copies are yours, so the first change after you
open it again asks once more and makes another copy. That is the price of the
Studio keeping no memory of its own - nothing you did is lost.

WHILE THE STUDIO IS OPEN, the player stops moving through presets by itself.
Otherwise the one under your hands would change mid-edit. The header says
"rotation held", and the button beside it starts it moving again.

If a preset does not load, a red bar appears at the top with the file and the
line - and the file tab marks that line.


4. Where it keeps its files
---------------------------

The player keeps its presets, settings and diagnostics.log in its own folder:

    Windows   %APPDATA%\Ritmolux\
    macOS     ~/Library/Application Support/Ritmolux/

That is the folder the Studio writes to, and the one the player watches. Paste
the path into Explorer, or use Finder's Go - Go to Folder.

The Studio's own settings are a separate, smaller file:

    Windows   %APPDATA%\ritmolux-studio\settings.json
    macOS     ~/Library/Application Support/ritmolux-studio/settings.json

You should not need to touch it. It has two keys: "playerMode", which the
settings panel writes for you, and "playerPath", which points the Studio at a
different player than the bundled one.

Deleting either folder resets that half; both are recreated on the next launch.


5. What to send back
--------------------

A few things, however roughly:

  - Did it open, and what did SmartScreen or macOS do?
  - Did you get two windows, and did the picture react to music?
  - Did the colours look right, or did anything look oddly blue or orange?
  - Did you manage to move a slider and see the picture change?
  - Did a saved change survive - does the look come back when you switch away
    and back to it in the library tab?
  - What the bottom strip reads after a few minutes, especially the dropped
    count.
  - Anything you went looking for and could not find.

That last one is the most useful. Thank you - this build exists to find out
what is missing.
