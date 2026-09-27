<#
.SYNOPSIS
The one build script for topside: builds, tests, and runs the 2D game,
with every log in logs/.
.DESCRIPTION
build and run stop the running game first (a running game locks
topside.exe); test never does, it tests the running game. Every action
runs cargo through k3sc cargo-lock from the repo root,
writes the full output to a log in logs/, and prints only what matters.
Every cargo run is watched: when its log has not grown for
StallMinutes, the script stops it and shows where it stalled instead of
hanging.
.PARAMETER Action
run: build, then launch the game detached in the background and return
at once (logs/topside.log, logs/topside.err.log).
test: run the tests (the topside-tests crate) against the running game
through cargo-nextest (.config/nextest.toml), all of them or one
tests/<Name>.rs; every test's time is printed, the run is in
logs/test.log, and every test's time and output in
logs/nextest/default/junit.xml.
build: build only (logs/build.log).
.PARAMETER Name
For test: one test file under tests/, without .rs. Empty runs every
test.
.PARAMETER StallMinutes
Minutes without new output before a cargo run counts as stalled.
.PARAMETER GameArgs
For run: passed to the game, for example --world labyrinth.
.EXAMPLE
pwsh -NoProfile -File scripts/build.ps1 test
.EXAMPLE
pwsh -NoProfile -File scripts/build.ps1 test game
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
        [Parameter(Mandatory = $true)][string]$Log,
        # Add to the log instead of starting it fresh.
        [switch]$Append
    )
    if (-not $Append) { Remove-Item $Log -ErrorAction SilentlyContinue }
    Write-Host "[build] k3sc cargo-lock $Arguments (log: $Log)"
    $p = Start-Process cmd -ArgumentList "/c k3sc cargo-lock $Arguments >> `"$Log`" 2>&1" -NoNewWindow -PassThru
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

switch ($Action) {
    'build' {
        Stop-Game
        $log = Join-Path $logs 'build.log'
        $code = Invoke-Cargo 'build' $log
        Show-Problems $log
    }
    'test' {
        $log = Join-Path $logs 'test.log'
        # cargo-nextest (.config/nextest.toml): every test in its own
        # process, its time printed, its output kept in
        # logs/nextest/default/junit.xml, a hung test stopped. The tests
        # are the topside-tests crate and test the running game, so the
        # game is left running.
        $targets = if ($Name) { "-p topside-tests -E test(/^$Name::/)" } else { '-p topside-tests' }
        $code = Invoke-Cargo "nextest run $targets" $log
        Show-Problems $log
        Select-String -Path $log -Pattern '^\s+(PASS|FAIL|SLOW|TERMINATING|TIMEOUT|SIGSEGV|SIGABRT|ABORT|LEAK|SKIP)\b|^\s+Summary|^\s+Starting|^error: ' |
            ForEach-Object {
                $color = if ($_.Line -match 'FAIL|TIMEOUT|TERMINATING|SIG|ABORT|^error') { 'Red' } elseif ($_.Line -match 'SLOW') { 'Yellow' } else { 'Gray' }
                Write-Host $_.Line -ForegroundColor $color
            }
        Write-Host "[build] every test's time and output: $(Join-Path $logs 'nextest\default\junit.xml')"
    }
    'run' {
        Stop-Game
        $log = Join-Path $logs 'build.log'
        $code = Invoke-Cargo 'build' $log
        Show-Problems $log
        if ($code -eq 0) {
            $out = Join-Path $logs 'topside.log'
            $err = Join-Path $logs 'topside.err.log'
            # Launched detached through WMI so the game inherits none of
            # this script's handles: a Start-Process child with redirected
            # output keeps the caller's pipe open, and whoever ran this
            # script waits until the game closes.
            $command = "cmd /c `"`"$exe`" $($GameArgs -join ' ') > `"$out`" 2> `"$err`"`""
            $launched = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
                CommandLine      = $command
                CurrentDirectory = $repo
            }
            if ($launched.ReturnValue -ne 0) {
                Write-Host "[build] launching topside FAILED (Win32_Process.Create returned $($launched.ReturnValue))" -ForegroundColor Red
                $code = 1
            } else {
                Write-Host "[build] topside launched in the background $GameArgs (log: $out, $err)" -ForegroundColor Green
            }
        }
    }
}
if ($code -eq 0) {
    Write-Host "[build] $Action ok" -ForegroundColor Green
} else {
    Write-Host "[build] $Action FAILED (exit $code, full log in logs/)" -ForegroundColor Red
}
exit $code
