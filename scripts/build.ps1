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
queue: run the tests named in Name through the running game's test
queue (authority.md "Running tests", tests/queue.rs): queued on the game,
run one at a time, each result reported to the game's `tests` op; fails
when any test failed. The run is in logs/queue.log.
build: build, then run the content crate's own tests (topside-content:
every episode file reads, the generic brain); either failing fails it
(logs/build.log).
.PARAMETER Name
For test: one test file under tests/, without .rs. Empty runs every
test. For queue: the first test file or full test name to queue; more
follow it, separated by spaces (build.ps1 queue the_tap leading queues
every test in tests/the_tap.rs and tests/leading.rs).
.PARAMETER StallMinutes
Minutes without new output before a cargo run counts as stalled.
.PARAMETER Port
The game's control plane port: passed to the game on run (--port), and
the port test and queue talk to (TOPSIDE_PORT). Two clones run two games
side by side on two ports.
.PARAMETER GameArgs
For run: passed to the game, for example --world labyrinth. For queue:
the tests to queue after the first.
.PARAMETER Renderer
For run: choose default, vulkan or dx12 for a renderer comparison.
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
    [ValidateSet('run', 'test', 'queue', 'build')]
    [string]$Action = 'run',
    [Parameter(Position = 1)]
    [string]$Name = '',
    [int]$StallMinutes = 5,
    [int]$Port = 15703,
    [ValidateSet('default', 'vulkan', 'dx12')]
    [string]$Renderer = 'default',
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$GameArgs = @()
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$logs = Join-Path $repo 'logs'
# The build's artifacts are in the repo's own target directory.
$exe = Join-Path $repo 'target\release\topside.exe'
New-Item -ItemType Directory -Force $logs | Out-Null
Set-Location $repo
# The tests talk to the game on this port (tests/endpoint.rs `url`).
$env:TOPSIDE_PORT = "$Port"

function Stop-Game {
    # Only the game this repo built and launched: the topside.exe running
    # from this repo's own target directory. Another clone's game (another
    # session's, on its own port) runs another exe and is left alone.
    Get-CimInstance Win32_Process -Filter "Name = 'topside.exe'" |
        Where-Object { $_.ExecutablePath -eq $exe } |
        ForEach-Object { taskkill /F /PID $_.ProcessId 2>&1 | Out-Null }
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
    # Waiting for another session's build (k3sc cargo-lock's lock) is
    # waiting, not stalling: the stall clock starts once the lock is held.
    $locked = $false
    while (-not $p.WaitForExit(5000)) {
        $size = if (Test-Path $Log) { (Get-Item $Log).Length } else { 0 }
        if (-not $locked) {
            $locked = (Test-Path $Log) -and (Select-String -Path $Log -Pattern 'acquired lock' -SimpleMatch -Quiet)
            if (-not $locked) {
                $quietSince = Get-Date
                continue
            }
        }
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
        $code = Invoke-Cargo 'build --release' $log
        # Built, the content's own tests (the topside-content crate: every
        # episode file reads through the loader, the generic brain): a data
        # file that does not read or a brain rule broken fails the build.
        if ($code -eq 0) {
            $code = Invoke-Cargo 'test -p topside-content --lib' $log -Append
            Select-String -Path $log -Pattern '^test |^test result:' | ForEach-Object {
                $color = if ($_.Line -match 'FAILED') { 'Red' } else { 'Gray' }
                Write-Host $_.Line -ForegroundColor $color
            }
        }
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
    'queue' {
        $log = Join-Path $logs 'queue.log'
        if (-not $Name) {
            Write-Host '[build] queue needs the tests to queue, for example: build.ps1 queue the_tap leading' -ForegroundColor Red
            exit 1
        }
        # The runner (tests/queue.rs) is ignored so a whole run never
        # starts it; here it is the one test run, with its output kept.
        $env:TOPSIDE_QUEUE = (@($Name) + $GameArgs) -join ' '
        $code = Invoke-Cargo 'nextest run -p topside-tests --no-capture --run-ignored only -E test(=queue::run_the_queue)' $log
        Show-Problems $log
        Select-String -Path $log -Pattern '^run \d+:|^running |^\s+(passed|FAILED)$|^(queued|running|passed|failed)\s+\S|^error: ' |
            ForEach-Object {
                $color = if ($_.Line -match 'FAILED|^failed|^error') { 'Red' } else { 'Gray' }
                Write-Host $_.Line -ForegroundColor $color
            }
    }
    'run' {
        Stop-Game
        $log = Join-Path $logs 'build.log'
        $code = Invoke-Cargo 'build --release' $log
        Show-Problems $log
        if ($code -eq 0) {
            $out = Join-Path $logs 'topside.log'
            $err = Join-Path $logs 'topside.err.log'
            # Launched detached through WMI so the game inherits none of
            # this script's handles: a Start-Process child with redirected
            # output keeps the caller's pipe open, and whoever ran this
            # script waits until the game closes.
            # The hidden PowerShell helper sets the renderer for this launch
            # and waits for the game so its redirected output stays connected.
            # Only the game's own window is visible.
            $launch = if ($Renderer -eq 'default') { '' } else { "`$env:WGPU_BACKEND = '$Renderer'`n" }
            $quotedArgs = @('--port', "$Port") + $GameArgs | ForEach-Object { "'" + $_.Replace("'", "''") + "'" }
            $launch += "Start-Process -FilePath '" + $exe.Replace("'", "''") + "' -ArgumentList @(" + ($quotedArgs -join ',') + ") -NoNewWindow -Wait -RedirectStandardOutput '" + $out.Replace("'", "''") + "' -RedirectStandardError '" + $err.Replace("'", "''") + "'"
            $launch = "`$ErrorActionPreference = 'Stop'`ntry {`n$launch`n} catch { `$_ | Out-String | Set-Content -LiteralPath '" + (Join-Path $logs 'launch.err.log').Replace("'", "''") + "'; exit 1 }"
            $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($launch))
            $command = "pwsh -NoProfile -EncodedCommand $encoded"
            $startup = New-CimInstance -CimClass (Get-CimClass -ClassName Win32_ProcessStartup) -ClientOnly -Property @{ ShowWindow = [uint16]0 }
            $launched = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
                CommandLine               = $command
                CurrentDirectory          = $repo
                ProcessStartupInformation = $startup
            }
            if ($launched.ReturnValue -ne 0) {
                Write-Host "[build] launching topside FAILED (Win32_Process.Create returned $($launched.ReturnValue))" -ForegroundColor Red
                $code = 1
            } else {
                Write-Host "[build] topside launched in the background on port $Port $GameArgs (log: $out, $err)" -ForegroundColor Green
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
