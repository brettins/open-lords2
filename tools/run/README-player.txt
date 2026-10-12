open-lords2
===========

An open reimplementation of the engine of Lords of the Realm II (Sierra /
Impressions, 1996). This download is the engine only: it contains none of the
original game's art, sound, music, films or data.

You need your own copy
----------------------

Install Lords of the Realm II first (GOG's edition works). open-lords2 reads
that folder and never writes to it. It looks, in order, at:

  1. a folder given on the command line: l2-game.exe "D:\Games\Lords of the Realm II"
  2. the LORDS2_DIR environment variable
  3. the folder it found last time
  4. its own folder, and the folder above it
  5. the GOG install recorded in the registry
  6. C:\GOG Games\Lords of the Realm II, GOG Galaxy's and Steam's game folders,
     C:\SIERRA\LORDS2
  7. otherwise it asks you to choose the folder

A folder counts if it holds L2_maps.dat and Lords2.exe (or L2.eng).

Where your files go
-------------------

Saves, settings (prefs.ini) and crash reports (crash-<date>-<time>.log) are in

  %APPDATA%\open-lords2\saves            on Windows
  ~/.local/share/open-lords2/saves       on Linux

Set LORDS2_SAVES to put them elsewhere. If the game stops unexpectedly, the
crash report there is what to attach to a bug report.

Keys
----

  Alt+Enter   switch between full screen (the default) and a window
  F5          in a window: snap it to a whole multiple of 640 x 480
  Ctrl+D      show or hide our debug overlay

Everything else is the original's controls; its manual applies.

Multiplayer
-----------

Multiple players opens the lobby: Host a game, or Join a game.

  Host     pick your shield and the map, press Start. The room shows a
           six-letter code; tell it to your friends. Start game lights up
           once everyone has pressed I'm ready.
  Join     type the code your host gave you and press Join, then I'm ready.

Codes go through our join-code service, which only pairs the two
connections; no ports to open. Without it, "use an address instead" on the
join page takes the host's address (TCP port 4712 by default; the game asks
the host's router to open it, or forward it by hand). --relay host:port or
LORDS2_RELAY picks another service.

Both players need the same build of this engine and their own copy of the game.

Command line
------------

  l2-game [<game folder>] [--mods <dir>] [--no-sound]
          [--host [port] | --join <addr|code>] [--relay <host:port>]
          [--players <n>] [--name <text>] [--shield <1-5>]

  --host opens the lobby hosting at once; --join opens it joining that host,
  by address or by code. With --players, a host presses Start when that many
  seats are ready and a joiner presses Ready at once, so a shortcut can carry
  a friend straight in.

On Linux the binary is l2-game; give it the game folder the same way:

  ./l2-game "/path/to/Lords of the Realm II"

Licence
-------

MIT; see LICENSE. See NOTICE for the rights in the original game.
