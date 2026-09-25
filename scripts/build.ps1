<#
.SYNOPSIS
The one build script for topside: builds, tests, and runs the 2D game,
with every log in logs/.
.DESCRIPTION
Every action stops the running game first (a running game locks
topside.exe), runs cargo through k3sc cargo-lock from the repo root,
writes the full output to a log in logs/, and prints only what matters.
Every cargo run is watched: when its log has not grown for
StallMinutes, the script stops it and shows where it stalled instead of
hanging.
.PARAMETER Action
run: build, then launch the game (logs/topside.log, logs/topside.err.log).
test: run the tests, all of them or one tests/<Name>.rs (logs/test.log).
build: build only (logs/build.log).
.PARAMETER Name
For test: one test file under tests/, without .rs. Empty runs every
test, the library's included.
.PARAMETER StallMinutes
Minutes without new output before a cargo run counts as stalled.
.PARAMETER GameArgs
For run: passed to the game, for example --world labyrinth.
.EXAMPLE
pwsh -NoProfile -File scripts/build.ps1 test
.EXAMPLE
pwsh -NoProfile -File scripts/build.ps1 test melee
.EXAMPLE
pwsh -NoProfile -File scripts/build.ps1 run
.NOTES
Never build, test, or launch topside any other way.
#>
[CmdletBinding()]
param (
    [Parameter(Position = 0)]
    [ValidateSet('run', 'test', 'build')]
    [string]$Action = 'run',
    [Parameter(Position = 1)]
    [string]$Name = '',
    [int]$StallMinutes = 5,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$GameArgs = @()
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$logs = Join-Path $repo 'logs'
# k3sc cargo-lock routes artifacts to the shared target directory.
$exe = 'C:\code\endless\rust\target\debug\topside.exe'
New-Item -ItemType Directory -Force $logs | Out-Null
Set-Location $repo

function Stop-Game {
    # taskkill errors when no game is running; that is fine.
    taskkill /F /IM topside.exe 2>&1 | Out-Null
}

function Invoke-Cargo {
    <#
    .SYNOPSIS
    Runs one cargo command through k3sc cargo-lock into a log, watched
    for stalls. Returns the exit code.
    #>
    param (
        [Parameter(Mandatory = $true)][string]$Arguments,
        [Parameter(Mandatory = $true)][string]$Log
    )
    Remove-Item $Log -ErrorAction SilentlyContinue
    Write-Host "[build] k3sc cargo-lock $Arguments (log: $Log)"
    $p = Start-Process cmd -ArgumentList "/c k3sc cargo-lock $Arguments > `"$Log`" 2>&1" -NoNewWindow -PassThru
    # Cache the handle so ExitCode is readable after the exit.
    $null = $p.Handle
    $lastSize = -1
    $quietSince = Get-Date
    while (-not $p.WaitForExit(5000)) {
        $size = if (Test-Path $Log) { (Get-Item $Log).Length } else { 0 }
        if ($size -ne $lastSize) {
            $lastSize = $size
            $quietSince = Get-Date
            continue
        }
        if (((Get-Date) - $quietSince).TotalMinutes -ge $StallMinutes) {
            Write-Host "[build] STALLED: no output for $StallMinutes minutes; stopping it" -ForegroundColor Red
            taskkill /T /F /PID $p.Id 2>&1 | Out-Null
            Write-Host '[build] the last lines before the stall:' -ForegroundColor Red
            Get-Content $Log -Tail 15 | ForEach-Object { Write-Host "  $(Limit-Line $_)" }
            return 124
        }
    }
    return $p.ExitCode
}

function Limit-Line {
    # Linker errors carry command lines thousands of characters long;
    # the full text stays in the log.
    param ([string]$Line)
    if ($Line.Length -le 300) { return $Line }
    return $Line.Substring(0, 300) + ' ...'
}

function Show-Problems {
    param ([Parameter(Mandatory = $true)][string]$Log)
    Select-String -Path $Log -Pattern '^(error|warning)(\[|:)' -Context 0, 6 |
        ForEach-Object {
            ($_.ToString() -split "`n") | ForEach-Object { Write-Host (Limit-Line $_) -ForegroundColor Yellow }
        }
}

Stop-Game
switch ($Action) {
    'build' {
        $log = Join-Path $logs 'build.log'
        $code = Invoke-Cargo 'build' $log
        Show-Problems $log
    }
    'test' {
        $log = Join-Path $logs 'test.log'
        $which = if ($Name) { "--test $Name" } else { '' }
        $code = Invoke-Cargo "test -p topside $which" $log
        Show-Problems $log
        Select-String -Path $log -Pattern '^test |^test result|panicked|^failures' |
            ForEach-Object {
                $color = if ($_.Line -match 'FAILED|panicked|failed; [1-9]') { 'Red' } else { 'Gray' }
                Write-Host $_.Line -ForegroundColor $color
            }
    }
    'run' {
        $log = Join-Path $logs 'build.log'
        $code = Invoke-Cargo 'build' $log
        Show-Problems $log
        if ($code -eq 0) {
            $out = Join-Path $logs 'topside.log'
            $err = Join-Path $logs 'topside.err.log'
            $launch = @{
                FilePath               = $exe
                WorkingDirectory       = $repo
                RedirectStandardOutput = $out
                RedirectStandardError  = $err
            }
            if ($GameArgs.Count -gt 0) { $launch.ArgumentList = $GameArgs }
            Start-Process @launch
            Write-Host "[build] topside launched $GameArgs (log: $out, $err)" -ForegroundColor Green
        }
    }
}
if ($code -eq 0) {
    Write-Host "[build] $Action ok" -ForegroundColor Green
} else {
    Write-Host "[build] $Action FAILED (exit $code, full log in logs/)" -ForegroundColor Red
}
exit $code
