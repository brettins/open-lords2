@echo off
REM Desktop-shortcut shim for play.ps1 - build the game, then run a COPY of it,
REM so a build never collides with a running game.
REM
REM Point a desktop shortcut at THIS file to get that behaviour. An existing
REM shortcut aimed straight at target\debug\l2-game.exe is unaffected and keeps
REM working: this adds a way to launch, it does not change where the binary is
REM built or what it is called.
REM
REM   play.cmd              debug build, then play
REM   play.cmd -Release     release build, then play
REM   play.cmd -NoBuild     skip the build, play what is there
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0play.ps1" %*
