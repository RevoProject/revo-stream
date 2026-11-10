# Smoke-tests the packaged GPUI engine process on Windows: it must bind its
# loopback endpoint, publish it, accept a connection, push its boot snapshot and
# answer a snapshot request, then exit cleanly once the client disconnects. The
# UI process needs a GPU desktop session, so it is not started here.
param(
    [string]$Root = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)))
)

$ErrorActionPreference = "Stop"
Set-Location $Root

$BundleDir = "crates\revo-gpui\target\release\bundle"
$Version = (Get-Content "crates\revo-gpui\Cargo.toml" | Select-String -Pattern '^version = "([^"]+)"' |
    Select-Object -First 1).Matches.Groups[1].Value
if (-not $Version) { throw "could not read the version" }
$Stage = Join-Path $BundleDir "revo-gpui-$Version-windows-x64"
$Engine = Join-Path $Stage "revo-engine.exe"
if (-not (Test-Path $Engine)) { throw "missing $Engine" }

$EndpointFile = Join-Path $env:RUNNER_TEMP "revo-engine-smoke-$PID.endpoint"
$Stdout = Join-Path $env:RUNNER_TEMP "revo-engine-smoke-$PID.log"
if (Test-Path $EndpointFile) { Remove-Item -Force $EndpointFile }

# The engine binary imports obs.dll, which the bundle stages inside revo-root;
# expose it through PATH so the loader resolves it before the process starts.
$env:PATH = (Join-Path $Stage "revo-root\core\lib") + ";" + $env:PATH

function Read-Exact([System.IO.Stream]$stream, [int]$count) {
    $buffer = New-Object byte[] $count
    $filled = 0
    while ($filled -lt $count) {
        $read = $stream.Read($buffer, $filled, $count - $filled)
        if ($read -le 0) { throw "engine closed the stream after $filled of $count bytes" }
        $filled += $read
    }
    return ,$buffer
}

function Read-Frame([System.IO.Stream]$stream) {
    $header = Read-Exact $stream 4
    $length = [System.BitConverter]::ToInt32($header, 0)
    if ($length -le 0) { throw "engine frame is not a length prefix ($length)" }
    $payload = Read-Exact $stream $length
    return [System.Text.Encoding]::UTF8.GetString($payload)
}

Write-Host "=== Starting the packaged engine ==="
$process = Start-Process -FilePath $Engine `
    -ArgumentList @("--endpoint-file", $EndpointFile) `
    -WorkingDirectory $Stage `
    -RedirectStandardOutput $Stdout `
    -PassThru

try {
    $deadline = (Get-Date).AddSeconds(30)
    while (-not (Test-Path $EndpointFile) -and (Get-Date) -lt $deadline) {
        if ($process.HasExited) { throw "engine exited with $($process.ExitCode):`n$(Get-Content $Stdout -Raw)" }
        Start-Sleep -Milliseconds 200
    }
    if (-not (Test-Path $EndpointFile)) { throw "engine never published its endpoint" }

    $endpoint = (Get-Content $EndpointFile -Raw).Trim()
    Write-Host "  endpoint: $endpoint"
    if ($endpoint -notmatch '^tcp:127\.0\.0\.1:(\d+)$') { throw "unexpected endpoint form: $endpoint" }
    $port = [int]$Matches[1]

    $client = New-Object System.Net.Sockets.TcpClient
    $client.Connect("127.0.0.1", $port)
    Write-Host "  loopback connection accepted"

    $stream = $client.GetStream()
    $stream.ReadTimeout = 20000

    $boot = Read-Frame $stream
    if ($boot -notmatch '"Snapshot"') { throw "boot frame is not a snapshot: $boot" }
    Write-Host "  boot snapshot received"

    $request = [System.Text.Encoding]::UTF8.GetBytes('"RequestSnapshot"')
    $stream.Write([System.BitConverter]::GetBytes($request.Length), 0, 4)
    $stream.Write($request, 0, $request.Length)
    $stream.Flush()

    $reply = Read-Frame $stream
    if ($reply -notmatch '"Snapshot"') { throw "snapshot reply is not a snapshot: $reply" }
    Write-Host "  snapshot round-trip ok"

    if ($process.HasExited) { throw "engine exited while the client was connected" }

    # The engine serves one client and exits once that client disconnects, so a
    # clean exit here is the expected outcome.
    $client.Close()
    $exitDeadline = (Get-Date).AddSeconds(15)
    while (-not $process.HasExited -and (Get-Date) -lt $exitDeadline) { Start-Sleep -Milliseconds 100 }
    if (-not $process.HasExited) { throw "engine stayed alive after the client disconnected" }
    Write-Host "  engine exited cleanly on disconnect"
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    Remove-Item -Force $EndpointFile -ErrorAction SilentlyContinue
}

Write-Host "GPUI engine smoke test passed"
