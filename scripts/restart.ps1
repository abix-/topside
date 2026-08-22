# The one lifecycle script: stop the running game, build, launch.
# Never hand-copy or hand-launch the exe; use this.
$repo = Split-Path -Parent $PSScriptRoot
# k3sc cargo-lock routes artifacts to the shared target directory.
$exe = "C:\code\endless\rust\target\debug\topside.exe"

# taskkill errors when no game is running; that is fine.
taskkill /F /IM topside.exe 2>&1 | Out-Null
$ErrorActionPreference = "Stop"

Set-Location $repo
k3sc cargo-lock build
if ($LASTEXITCODE -ne 0) {
    Write-Host "build failed, not launching" -ForegroundColor Red
    exit 1
}

# Arguments pass through to the exe: restart.ps1 --world labyrinth
if ($args.Count -gt 0) {
    Start-Process $exe -WorkingDirectory $repo -ArgumentList $args
} else {
    Start-Process $exe -WorkingDirectory $repo
}
Write-Host "topside launched $args"
