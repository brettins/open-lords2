<#
  Runs scenario scripts (tools/oracle/scenarios/*.txt) on the hidden original with the event
  ring on, -Runs times each: <name>.raw.jsonl (every event) and <name>.orig.jsonl (FORMAT.md records,
  by trace-shape.js); later runs add .run<k>. A .jsonl scenario is FORMAT.md steps.
  Prints one JSON line per scenario: steps done, event counts, identical across runs or not.
  Script lines (one step each, '#' comments):
    films play|skip          before any step; default play
    fixture <file.sav>       the only *.sav in the install at launch (from -Fixtures)
    load                     title menu > Load > first entry > OK, then waits for the map
    new-game original|new    Play Now! > campaign > Continue, then 300 frames
    click x y | rclick x y | key <vk number or name> | advance N | snap | anchor <label>
    move x y | drag x y x2 y2 | record N    (record: N frames keeping draw records)
    wait-screen N | wait-page N | wait-phase N | wait-idle N   (numbers may be 0x..)
    wait-screen-not N | wait-page-not N | peek <addr>  (a dword of the game, as a note)
    dismiss                  press OK on message boxes (screen 0x27) until none is up
    ok                       press the Ui_OkButton 0x0040D1BC drawn next frame; fails if none is
  snap keeps the draw-leaf records of the next frame; other frames only count draws.
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string[]]$Scenario,
  [string]$Install = 'E:\games\l2-trace',
  [string]$Out = 'C:\Temp\l2-trace-out',
  [string]$Fixtures = 'E:\dev\lords2-fixtures',
  [string]$Dlls = 'C:\Temp\l2-trace-build',
  [int]$Runs = 2,
  [string]$Dump = '',
  [int]$TimeoutSec = 300
)
$ErrorActionPreference = 'Stop'
if ([IO.Path]::GetFullPath($Install) -match '^[Ff]:') { throw "refusing an F:\ install (read-only)" }
. (Join-Path $PSScriptRoot 'trace.ps1') -Install $Install -Dump $Dump -TimeoutSec $TimeoutSec
$table = Join-Path $PSScriptRoot '..\..\native\l2-trace\hooktable.json'
[L2Trace]::Hooks.Clear()
foreach ($h in (Get-Content -Raw $table | ConvertFrom-Json)) {
  [L2Trace]::Hooks[[Convert]::ToUInt32($h.addr, 16)] = [string[]]@($h.name, $h.cat)
}
New-Item -ItemType Directory -Force $Out | Out-Null

function Num([string]$s) { if ($s -match '^0x') { [Convert]::ToInt32($s.Substring(2), 16) } else { [int]$s } }
$vk = @{ enter = 13; return = 13; escape = 27; esc = 27; space = 32; left = 37; up = 38; right = 39; down = 40; tab = 9 }

# Front-end pages 1 and 2 put items 192 x 24 at x 0xE0, y 0x5B + 36 n (screens-county.md 1.2); Load is
# page 2 item 2 ("Load a game", measured). Page 3's list starts at (0x60 + 0x20, 0x0A + 0x6C) (10.7); the thumb-up
# (OK) at (416, 88) is measured from a frame dump. The caret blinks, so page 3 never idles.
# SaveLoad_Tick 0x004AD9F0 copies 12 bytes of the name (0x004EA130) and opens it (FUN_004AF767):
# a miss sets the countdown DAT_0057D3C4 to 400, a hit to 0x96 and then calls Save_Read.
$LoadSteps = @('wait-page 1', 'wait-idle 5', 'click 320 103', 'wait-page 2', 'wait-idle 5', 'click 320 139', 'wait-page 3',
  'advance 20', 'click 416 88', 'wait-event call:save', 'wait-screen-not 0x1F', 'advance 60')

# Page 2 "Play Now!" (item 1) -> page 5, 39.4 / 39.5 in 160-wide boxes at x 112 / 368, y 117 -> page 4,
# "Continue" at (336, 215) 192 x 24; all measured from the trace's own draw records.
$NewGameSteps = @('wait-page 1', 'wait-idle 5', 'click 320 103', 'wait-page 2', 'wait-idle 5', 'click 320 103', 'wait-page 5', 'advance 20')
$NewGameTail = @('wait-page 4', 'advance 20', 'click 432 227', 'wait-screen-not 0x1F', 'advance 300')

# One FORMAT.md step (JSON) as a text step; films, wait-page, dismiss, peek and new-game's campaign are this side's.
function FromJson($line) {
  $o = $line | ConvertFrom-Json
  switch ($o.op) {
    'load' { "fixture $($o.save)"; 'load' }
    'new-game' { "new-game $(if ($o.campaign) { $o.campaign } else { 'original' })" }
    'click' { "$(if ($o.button -eq 'right') { 'rclick' } else { 'click' }) $($o.x) $($o.y)" }
    'key' { $k = [string]$o.key; if ($k.Length -eq 1) { "key $([int][char]$k.ToUpper())" } else { "key $($k.ToLower())" } }
    'drag' { "drag $($o.x) $($o.y) $($o.x2) $($o.y2)" }
    'move' { "move $($o.x) $($o.y)" }
    'record' { "record $($o.frames)" }
    'wait-event' { "wait-event $($o.event)" }
    'wait-screen-not' { "wait-screen-not $($o.id)" }
    'wait-screen' { "wait-screen $($o.id)" }
    'wait-phase' { "wait-phase $($o.phase)" }
    'wait-page' { "wait-page $($o.page)" }
    'wait-idle' { "wait-idle $($o.frames)" }
    'advance' { "advance $($o.frames)" }
    'anchor' { "anchor $($o.label)" }
    'peek' { "peek $($o.addr)" }
    'films' { "films $($o.mode)" }
    default { $o.op }
  }
}

function Run-Scenario($file, $run) {
  $name = [IO.Path]::GetFileNameWithoutExtension($file)
  $lines = Get-Content $file | ForEach-Object { ($_ -replace '#.*$', '').Trim() } | Where-Object { $_ }
  if ($file -like '*.jsonl') { $lines = @(Get-Content $file | Where-Object { $_.Trim() -and $_ -notmatch '^\s*//' } | ForEach-Object { FromJson $_ }) }
  $films = 'play'; $fixture = $null; $steps = @()
  foreach ($l in $lines) {
    $w = $l -split '\s+'
    switch ($w[0]) {
      'films' { $films = $w[1] }
      'fixture' { $fixture = $w[1] }
      'load' { $steps += $LoadSteps }
      'new-game' { $steps += $NewGameSteps; $steps += "click $(if ($w[1] -eq 'new') { 448 } else { 192 }) 129"; $steps += $NewGameTail }
      default { $steps += $l }
    }
  }
  & (Join-Path $PSScriptRoot 'trace-install.ps1') -Dest $Install -Dlls $Dlls | Out-Null
  Get-ChildItem $Install -Filter *.sav | Remove-Item -Force
  # The name field is 8.3: SaveLoad_Tick copies 12 bytes of it, so a longer name opens nothing.
  if ($fixture) { Copy-Item (Join-Path $Fixtures $fixture) (Join-Path $Install 'fixture.sav') }
  $sfx = if ($run -eq 1) { '' } else { ".run$run" }
  $jsonl = Join-Path $Out "$name.raw$sfx.jsonl"
  $shaped = Join-Path $Out "$name.orig$sfx.jsonl"
  $t = [L2Trace]::Launch($Install, 1, $Dump, $true, $jsonl, $films)
  $timeout = $TimeoutSec * 1000; $done = 0; $failed = $null
  # Start at the first gated present, so every note lands at the same point of the stream.
  while ($t.Frames -lt 1 -and -not $t.Proc.HasExited) { Start-Sleep -Milliseconds 5 }
  try {
    foreach ($s in $steps) {
      $w = $s -split '\s+'
      $t.Note('{"ev":"step","f":' + $t.Frames + ',"step":"' + $s + '"}')
      $ok = $true
      switch ($w[0]) {
        'click' { $t.Click((Num $w[1]), (Num $w[2])) }
        'rclick' { $t.RClick((Num $w[1]), (Num $w[2])) }
        'move' { $t.MoveTo((Num $w[1]), (Num $w[2])) }
        'drag' { $t.Drag((Num $w[1]), (Num $w[2]), (Num $w[3]), (Num $w[4])) }
        'record' { $t.KeepDraws = $true; $t.Advance((Num $w[1])); $t.KeepDraws = $false }
        'key' { $k = if ($vk.ContainsKey($w[1])) { $vk[$w[1]] } else { Num $w[1] }; $t.Key($k) }
        'advance' { $t.Advance((Num $w[1])) }
        'snap' { $t.KeepDraws = $true; $t.Advance(1); $t.KeepDraws = $false; $t.Note('{"ev":"snap","f":' + $t.Frames + ',"hash":"' + ('{0:x8}' -f $t.Hash) + '"}') }
        'anchor' { $t.Note('{"ev":"anchor","f":' + $t.Frames + ',"label":"' + ($w[1..9] -join ' ') + '"}') }
        'wait-screen' { $n = Num $w[1]; $ok = $t.WaitUntil({ $t.Screen -eq $n }.GetNewClosure(), $timeout) }
        'wait-screen-not' { $n = Num $w[1]; $ok = $t.WaitUntil({ $t.Screen -ne $n }.GetNewClosure(), $timeout) }
        'wait-page' { $n = Num $w[1]; $ok = $t.WaitUntil({ $t.Page -eq $n }.GetNewClosure(), $timeout) }
        'wait-page-not' { $n = Num $w[1]; $ok = $t.WaitUntil({ $t.Page -ne $n }.GetNewClosure(), $timeout) }
        'wait-phase' { $n = Num $w[1]; $ok = $t.WaitUntil({ $t.Phase -eq $n }.GetNewClosure(), $timeout) }
        'wait-idle' { $ok = $t.WaitIdle((Num $w[1]), $timeout) }
        'wait-event' { $k = $w[1]; $n0 = $t.CountOf($k); $ok = $t.WaitUntil({ $t.CountOf($k) -gt $n0 }.GetNewClosure(), $timeout) }
        # Ui_OkButton 0x0040D1BC draws the OK at (x, y); screen 0x27 is a message box up.
        'dismiss' { for ($i = 0; $i -lt 20 -and $t.Screen -eq 0x27; $i++) { $t.OkX = -1; $t.KeepDraws = $true; $t.Advance(1); $t.KeepDraws = $false; $t.Drain(); if ($t.OkX -lt 0) { break }; $t.Click($t.OkX + 24, $t.OkY + 16); $t.Advance(60) } }
        'ok' { $t.OkX = -1; $t.KeepDraws = $true; $t.Advance(1); $t.KeepDraws = $false; $t.Drain(); $ok = $t.OkX -ge 0; if ($ok) { $t.Click($t.OkX + 24, $t.OkY + 16) } }
        'peek' { $t.Note('{"ev":"peek","f":' + $t.Frames + ',"addr":"' + $w[1] + '","value":' + $t.Read((Num $w[1]), 4) + '}') }
        default { throw "unknown step: $s" }
      }
      if (-not $ok -or $t.Proc.HasExited) { $failed = $s; break }
      $done++
    }
    if ($failed) { $t.Note('{"ev":"failed","f":' + $t.Frames + ',"step":"' + $failed + '","screen":' + $t.Screen + ',"page":' + $t.Page + '}') }
    $t.Drain()
    $r = [ordered]@{
      scenario = $name; run = $run; steps = "$done/$($steps.Count)"; failed = $failed
      frames = $t.Frames; screen = $t.Screen; phase = $t.Phase; realMs = $t.Real.ElapsedMilliseconds
      refused = $t.Shared(44); pl8Draws = $t.SheetDraws; pl8Unnamed = $t.UnnamedSheets
      counts = $t.Counts
    }
  } finally { $t.Dispose() }
  node (Join-Path $PSScriptRoot 'trace-shape.js') $jsonl $shaped
  $r.sha = (Get-FileHash $jsonl -Algorithm SHA256).Hash.Substring(0, 12) + '/' + (Get-FileHash $shaped -Algorithm SHA256).Hash.Substring(0, 12)
  $r.records = (Get-Content $shaped | Measure-Object -Line).Lines
  $r
}

foreach ($file in $Scenario) {
  $results = @(1..$Runs | ForEach-Object { Run-Scenario $file $_ })
  $first = $results[0]
  $first.identical = @($results | Where-Object { $_.sha -ne $first.sha }).Count -eq 0
  $first.runs = $Runs
  $first | ConvertTo-Json -Compress -Depth 4
}
