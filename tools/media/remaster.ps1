<#
  Remaster the game's Smacker videos into a modern format, optionally with AI
  upscaling and frame interpolation.

  Why this exists, beyond looking nicer: there is no permissively licensed
  Smacker decoder (see docs/decisions.md D5a). Doing the decode ONCE here, in a
  separate process, on the user's own machine, keeps any copyleft obligation off
  our engine binary entirely - the engine only ever reads the output format.

  Nothing is ever distributed: the source videos are the user's own copy of the
  game, and the output lands outside both the repository and the game install.

  Stages (run all, or one at a time):
    extract   .smk -> PNG frames + WAV audio          [needs ffmpeg]
    upscale   frames -> larger frames                 [optional AI tool]
    encode    frames + audio -> .mp4                  [needs ffmpeg]

  Examples:
    .\remaster.ps1 -Stage all -Only Intro.smk
    .\remaster.ps1 -Stage all -Upscaler esrgan -Interpolate
    .\remaster.ps1 -Stage encode -Crf 16
#>
[CmdletBinding()]
param(
  [ValidateSet('all', 'extract', 'upscale', 'encode', 'probe')]
  [string]$Stage = 'all',

  [string]$Source = 'F:\games\Lords of the Realm II',
  [string]$Work   = 'F:\games\lords2-video',

  # Restrict to one file, e.g. -Only Intro.smk
  [string]$Only,

  [ValidateSet('none', 'lanczos', 'esrgan')]
  [string]$Upscaler = 'lanczos',
  [int]$Scale = 4,

  # Frame interpolation. The videos are 12 fps, which reads as judder on a
  # modern display. 'rife' is far better than 'ffmpeg' but needs the tool.
  [switch]$Interpolate,
  [ValidateSet('ffmpeg', 'rife')]
  [string]$Interpolator = 'ffmpeg',
  [int]$TargetFps = 48,

  [int]$Crf = 18,
  [string]$FFmpeg    = 'F:\ffmpeg\bin\ffmpeg.exe',
  [string]$Esrgan    = '',   # realesrgan-ncnn-vulkan.exe
  [string]$Rife      = ''    # rife-ncnn-vulkan.exe
)

$ErrorActionPreference = 'Stop'

# These three are stored at half vertical resolution and doubled at display time.
# Imptitle.smk is deliberately NOT here: at 500x292 it is already full height,
# and an earlier revision of this list wrongly stretched it to 500x584.
# Verified rather than assumed: sampling a frame of Intro.smk found 61 of 72
# adjacent row pairs identical and zero blank rows, so the Smacker Y-flag means
# "duplicate rows", not "interlace". Scaling Y by 2 here restores the geometry
# the game actually shows, and does it with a real filter rather than by
# repeating rows.
$YDoubled = @{
  'intro.smk'    = $true
  'credits.smk'  = $true
  'lom.smk'      = $true
}

function Require-Tool([string]$path, [string]$name, [string]$hint) {
  if (-not $path -or -not (Test-Path $path)) {
    Write-Host "$name not found." -ForegroundColor Yellow
    Write-Host "  $hint"
    return $false
  }
  return $true
}

if (-not (Require-Tool $FFmpeg 'ffmpeg' 'Pass -FFmpeg <path to ffmpeg.exe>.')) { exit 1 }
$ffprobe = Join-Path (Split-Path $FFmpeg) 'ffprobe.exe'

$videos = Get-ChildItem -Path $Source -Filter *.smk -File
if ($Only) { $videos = $videos | Where-Object { $_.Name -ieq $Only } }
if (-not $videos) { Write-Host "no .smk files matched"; exit 1 }

New-Item -ItemType Directory -Force $Work | Out-Null
$framesRoot = Join-Path $Work 'frames'
$upRoot     = Join-Path $Work 'upscaled'
$outRoot    = Join-Path $Work 'out'

# ---------------------------------------------------------------- probe ----
if ($Stage -eq 'probe') {
  foreach ($v in $videos) {
    $info = & $ffprobe -v error -select_streams v:0 `
      -show_entries stream=width,height,nb_read_packets,r_frame_rate `
      -count_packets -of csv=p=0 $v.FullName 2>$null
    $dbl = if ($YDoubled[$v.Name.ToLower()]) { ' (Y-doubled at display)' } else { '' }
    "{0,-16} {1}{2}" -f $v.Name, $info, $dbl
  }
  exit 0
}

# -------------------------------------------------------------- extract ----
if ($Stage -in 'all', 'extract') {
  foreach ($v in $videos) {
    $name = [IO.Path]::GetFileNameWithoutExtension($v.Name)
    $dir  = Join-Path $framesRoot $name
    New-Item -ItemType Directory -Force $dir | Out-Null

    # Restore true display geometry before anything else touches the pixels.
    $vf = if ($YDoubled[$v.Name.ToLower()]) { 'scale=iw:ih*2:flags=lanczos' } else { 'null' }

    Write-Host "extract $($v.Name)"
    & $FFmpeg -hide_banner -loglevel error -y -i $v.FullName `
      -vf $vf -start_number 0 (Join-Path $dir 'f%06d.png')
    # Audio is a separate stream; keep it lossless for re-muxing later.
    & $FFmpeg -hide_banner -loglevel error -y -i $v.FullName `
      -vn -acodec pcm_s16le (Join-Path $Work "$name.wav") 2>$null
  }
}

# -------------------------------------------------------------- upscale ----
if ($Stage -in 'all', 'upscale') {
  if ($Upscaler -eq 'esrgan') {
    $hint = 'Download realesrgan-ncnn-vulkan (standalone, no Python, works on any Vulkan GPU) ' +
            'and pass -Esrgan <path to exe>. Falling back to lanczos.'
    if (-not (Require-Tool $Esrgan 'realesrgan-ncnn-vulkan' $hint)) { $Upscaler = 'lanczos' }
  }

  foreach ($v in $videos) {
    $name = [IO.Path]::GetFileNameWithoutExtension($v.Name)
    $src  = Join-Path $framesRoot $name
    $dst  = Join-Path $upRoot $name
    if (-not (Test-Path $src)) { Write-Host "  no frames for $name - run extract first"; continue }
    New-Item -ItemType Directory -Force $dst | Out-Null

    switch ($Upscaler) {
      'esrgan' {
        Write-Host "upscale $name x$Scale (esrgan)"
        # realesr-animevideov3 suits hand-drawn and pre-rendered 1990s art far
        # better than the photographic models.
        & $Esrgan -i $src -o $dst -s $Scale -n realesr-animevideov3 -f png
      }
      'lanczos' {
        Write-Host "upscale $name x$Scale (lanczos)"
        & $FFmpeg -hide_banner -loglevel error -y -i (Join-Path $src 'f%06d.png') `
          -vf "scale=iw*${Scale}:ih*${Scale}:flags=lanczos" -start_number 0 `
          (Join-Path $dst 'f%06d.png')
      }
      'none' {
        Write-Host "upscale $name skipped"
        Copy-Item (Join-Path $src '*.png') $dst
      }
    }
  }
}

# --------------------------------------------------------------- encode ----
if ($Stage -in 'all', 'encode') {
  New-Item -ItemType Directory -Force $outRoot | Out-Null

  if ($Interpolate -and $Interpolator -eq 'rife') {
    $hint = 'Download rife-ncnn-vulkan (standalone) and pass -Rife <path to exe>. ' +
            'Falling back to ffmpeg minterpolate.'
    if (-not (Require-Tool $Rife 'rife-ncnn-vulkan' $hint)) { $Interpolator = 'ffmpeg' }
  }

  foreach ($v in $videos) {
    $name = [IO.Path]::GetFileNameWithoutExtension($v.Name)
    $src  = Join-Path $upRoot $name
    if (-not (Test-Path $src)) { $src = Join-Path $framesRoot $name }
    if (-not (Test-Path $src)) { Write-Host "  nothing to encode for $name"; continue }

    $wav = Join-Path $Work "$name.wav"
    $out = Join-Path $outRoot "$name.mp4"

    # Source rate is 12 fps for all but two files; ffprobe rather than assume.
    $rate = & $ffprobe -v error -select_streams v:0 -show_entries stream=r_frame_rate `
              -of csv=p=0 $v.FullName
    if (-not $rate) { $rate = '12/1' }

    $args = @('-hide_banner', '-loglevel', 'error', '-y',
              '-framerate', $rate, '-start_number', '0', '-i', (Join-Path $src 'f%06d.png'))
    if (Test-Path $wav) { $args += @('-i', $wav) }

    if ($Interpolate -and $Interpolator -eq 'ffmpeg') {
      # Slow, and not as good as RIFE, but needs no extra tool.
      $args += @('-vf', "minterpolate=fps=${TargetFps}:mi_mode=mci:mc_mode=aobmc:vsbmc=1")
    }

    $args += @('-c:v', 'libx264', '-preset', 'slow', '-crf', "$Crf",
               '-pix_fmt', 'yuv420p', '-movflags', '+faststart')
    if (Test-Path $wav) { $args += @('-c:a', 'aac', '-b:a', '128k', '-shortest') }
    $args += $out

    Write-Host "encode $name -> $([IO.Path]::GetFileName($out))"
    & $FFmpeg @args
  }

  Write-Host ""
  Write-Host "output: $outRoot"
  Get-ChildItem $outRoot -Filter *.mp4 -ErrorAction SilentlyContinue |
    ForEach-Object { "  {0,-20} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB) }
}
