# The one lifecycle script: stop the running game, build, launch.
# Never hand-copy or hand-launch the exe; use this.
$repo = Split-Path -Parent $PSScriptRoot
# k3sc cargo-lock routes artifacts to the shared target directory.
# The 3D game in 3d/ builds as topside-3d.exe.
$exe = "C:\code\endless\rust\target\debug\topside-3d.exe"

# taskkill errors when no game is running; that is fine.
taskkill /F /IM topside-3d.exe 2>&1 | Out-Null
$ErrorActionPreference = "Stop"

Set-Location $repo
k3sc cargo-lock build
if ($LASTEXITCODE -ne 0) {
    Write-Host "build failed, not launching" -ForegroundColor Red
    exit 1
}

# Arguments pass through to the exe: restart.ps1 --world labyrinth
# The game's output lands in topside.log (Bevy's asset and render
# errors included); the previous run's log is overwritten.
$out = Join-Path $repo "topside.log"
$err = Join-Path $repo "topside.err.log"
if ($args.Count -gt 0) {
    Start-Process $exe -WorkingDirectory $repo -ArgumentList $args -RedirectStandardOutput $out -RedirectStandardError $err
} else {
    Start-Process $exe -WorkingDirectory $repo -RedirectStandardOutput $out -RedirectStandardError $err
}
Write-Host "topside launched $args (log: topside.log, topside.err.log)"
