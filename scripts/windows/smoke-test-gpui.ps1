# Smoke-tests the packaged GPUI engine process on Windows: it must bind its
# loopback endpoint, publish it, and accept a connection. The UI process needs
# a GPU desktop session, so it is not started here.
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
    $stream.ReadTimeout = 10000
    $header = New-Object byte[] 4
    $read = $stream.Read($header, 0, 4)
    if ($read -ne 4) { throw "engine did not send its boot snapshot header" }
    $payloadLength = [System.BitConverter]::ToInt32($header, 0)
    if ($payloadLength -le 0) { throw "engine snapshot header is not a length prefix ($payloadLength)" }
    $received = 0
    while ($received -lt $payloadLength) {
        $chunk = $stream.Read($header, 0, [System.Math]::Min(4, $payloadLength - $received))
        if ($chunk -le 0) { break }
        $received += $chunk
    }
    Write-Host ("  boot snapshot frame received ({0} bytes)" -f $payloadLength)
    $client.Close()
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    Remove-Item -Force $EndpointFile -ErrorAction SilentlyContinue
}

Write-Host "GPUI engine smoke test passed"
