# Packages the native GPUI application for Windows x64: portable ZIP, NSIS
# installer (.exe) and WiX MSI. The staged tree is built once and shared by all
# three artifacts, so what the ZIP contains is exactly what the installers ship.
#
# Inputs (release build, plus the runtime staged by prepare-runtime.ps1):
#   crates/revo-gpui/target/release/revo-gpui.exe
#   crates/revo-backend/target/release/revo-engine.exe
#   src-tauri/resources/revo-root
#
# Outputs (crates/revo-gpui/target/release/bundle):
#   revo-gpui-<version>-windows-x64.zip
#   revo-gpui-<version>-windows-x64.exe   (NSIS)
#   revo-gpui-<version>-windows-x64.msi   (WiX)
param(
    [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [switch]$SkipMsi
)

$ErrorActionPreference = "Stop"
Set-Location $Root

$UiBin = "crates\revo-gpui\target\release\revo-gpui.exe"
$EngineBin = "crates\revo-backend\target\release\revo-engine.exe"
$RuntimeRoot = "src-tauri\resources\revo-root"
$BundleDir = "crates\revo-gpui\target\release\bundle"

if (-not (Test-Path $UiBin)) { throw "missing $UiBin" }
if (-not (Test-Path $EngineBin)) { throw "missing $EngineBin" }
if (-not (Test-Path "$RuntimeRoot\core\lib\obs-plugins")) { throw "missing staged runtime: $RuntimeRoot" }

$Version = (Get-Content "crates\revo-gpui\Cargo.toml" | Select-String -Pattern '^version = "([^"]+)"' |
    Select-Object -First 1).Matches.Groups[1].Value
if (-not $Version) { throw "could not read the version from crates/revo-gpui/Cargo.toml" }

$Stage = Join-Path $BundleDir "revo-gpui-$Version-windows-x64"
$Prefix = "revo-gpui-$Version-windows-x64"
New-Item -ItemType Directory -Force -Path $BundleDir | Out-Null
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
# makensis resolves script-relative paths against the .nsi file's own directory
# (the bundle), not the repository, so everything embedded below must be
# absolute before the NSIS/WiX sources are generated.
$BundleDir = (Resolve-Path $BundleDir).Path
$Stage = (Resolve-Path $Stage).Path

Write-Host "=== Staging application tree ($Version) ==="
Copy-Item $UiBin (Join-Path $Stage "revo-gpui.exe")
Copy-Item $EngineBin (Join-Path $Stage "revo-engine.exe")
Copy-Item $RuntimeRoot (Join-Path $Stage "revo-root") -Recurse
if (Test-Path "src-tauri\icons\icon.ico") {
    Copy-Item "src-tauri\icons\icon.ico" (Join-Path $Stage "revo-gpui.ico")
}
@"
RevoStream (native GPUI build) $Version

revo-gpui.exe    UI process (GPUI renderer, no libobs in this process)
revo-engine.exe  libobs runtime, spawned automatically by the UI
revo-root\       OBS runtime: core\lib, core\bin, core\share, data

Set REVO_ROOT to relocate the runtime, REVO_ENGINE_BIN to use another engine
process, REVO_DATA_DIR to relocate per-user settings and logs.
"@ | Set-Content -Encoding utf8 (Join-Path $Stage "README.txt")

Write-Host "=== Portable ZIP ==="
$Zip = Join-Path $BundleDir "$Prefix.zip"
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path $Stage -DestinationPath $Zip -CompressionLevel Optimal
if (-not (Test-Path $Zip)) { throw "portable ZIP was not produced" }
Write-Host "    $Zip"

function Find-Tool([string[]]$names, [string]$hint) {
    foreach ($name in $names) {
        $command = Get-Command $name -ErrorAction SilentlyContinue
        if ($command) { return $command.Source }
    }
    foreach ($candidate in @(
        "$env:ProgramFiles\NSIS\makensis.exe",
        "$env:ProgramFiles (x86)\NSIS\makensis.exe",
        "$env:LOCALAPPDATA\chocolatey\lib\nsis\tools\makensis.exe",
        "$env:ProgramFiles\WiX Toolset v3.11\bin\candle.exe",
        "$env:ProgramFiles\WiX Toolset v3.14\bin\candle.exe",
        "${env:ProgramFiles(x86)}\WiX Toolset v3.14\bin\candle.exe",
        "${env:ProgramFiles(x86)}\WiX Toolset v3.11\bin\candle.exe"
    )) {
        if (Test-Path $candidate) { return $candidate }
    }
    Write-Warning "tool not found ($($names -join ', ')): $hint"
    return $null
}

Write-Host "=== NSIS installer ==="
$MakeNsis = Find-Tool @("makensis", "makensis.exe") "install NSIS (choco install nsis)"
if ($MakeNsis) {
    $Nsi = Join-Path $BundleDir "revo-gpui.nsi"
    $IconLine = if (Test-Path (Join-Path $Stage "revo-gpui.ico")) { "!define MUI_ICON `"$(Join-Path $Stage 'revo-gpui.ico')`"" } else { "" }
    @"
!include "MUI2.nsh"
!define PRODUCT_NAME "RevoStream"
!define PRODUCT_VERSION "$Version"
Name "`${PRODUCT_NAME} `${PRODUCT_VERSION}"
OutFile "`${OUTFILE}"
Unicode true
InstallDir "`$LOCALAPPDATA`\Programs\RevoStream"
RequestExecutionLevel user
$IconLine
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "RevoStream"
    SetShellVarContext current
    SetOutPath "`$INSTDIR"
    File /r "$Stage\*"
    CreateShortCut "`$SMPROGRAMS\RevoStream.lnk" "`$INSTDIR\revo-gpui.exe"
    CreateShortCut "`$DESKTOP\RevoStream.lnk" "`$INSTDIR\revo-gpui.exe"
    WriteUninstaller "`$INSTDIR\uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RevoStream" "DisplayName" "RevoStream"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RevoStream" "DisplayVersion" "`${PRODUCT_VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RevoStream" "UninstallString" "`$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
    SetShellVarContext current
    RMDir /r "`$INSTDIR"
    Delete "`$SMPROGRAMS\RevoStream.lnk"
    Delete "`$DESKTOP\RevoStream.lnk"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RevoStream"
SectionEnd
"@ | Set-Content -Encoding utf8 $Nsi
    $NsisOut = Join-Path $BundleDir "$Prefix.exe"
    if (Test-Path $NsisOut) { Remove-Item -Force $NsisOut }
    & $MakeNsis "-DOUTFILE=$NsisOut" $Nsi
    if ($LASTEXITCODE -ne 0) { throw "makensis failed with $LASTEXITCODE" }
    if (-not (Test-Path $NsisOut)) { throw "NSIS installer was not produced" }
    Write-Host "    $NsisOut"
}

Write-Host "=== WiX MSI ==="
$Candle = Find-Tool @("candle.exe") "install WiX Toolset v3 (choco install wixtoolset)"
if ($Candle -and -not $SkipMsi) {
    $WixBin = Split-Path -Parent $Candle
    $Light = Join-Path $WixBin "light.exe"
    $Heat = Join-Path $WixBin "heat.exe"
    if (-not (Test-Path $Light) -or -not (Test-Path $Heat)) { throw "WiX toolset is incomplete next to candle.exe: $WixBin" }

    $Fragment = Join-Path $BundleDir "payload.wxs"
    & $Heat dir $Stage -sfrag -srd -scom -sreg -gg -cg Payload -dr INSTALLFOLDER -var var.PayloadSource -out $Fragment
    if ($LASTEXITCODE -ne 0) { throw "heat harvest failed with $LASTEXITCODE" }

    $Product = Join-Path $BundleDir "product.wxs"
    @"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="RevoStream" Language="1033" Version="$Version"
           Manufacturer="RevoProject" UpgradeCode="6f0b4a1c-9f4d-4b17-9d3e-7c1c5a90b1aa">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perUser" Platform="x64"/>
    <MajorUpgrade DowngradeErrorMessage="A newer version of RevoStream is already installed."/>
    <MediaTemplate EmbedCab="yes" CompressionLevel="high"/>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="LocalAppDataFolder">
        <Directory Id="ProgramsFolder" Name="Programs">
          <Directory Id="INSTALLFOLDER" Name="RevoStream"/>
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Directory Id="ApplicationShortcutDir" Name="RevoStream"/>
      </Directory>
    </Directory>
    <DirectoryRef Id="ApplicationShortcutDir">
      <Component Id="ApplicationShortcut" Guid="*">
        <Shortcut Id="RevoStreamShortcut" Name="RevoStream" Target="[INSTALLFOLDER]revo-gpui.exe"
                  WorkingDirectory="INSTALLFOLDER"/>
        <RemoveFolder Id="ApplicationShortcutDir" On="uninstall"/>
        <RegistryValue Root="HKCU" Key="Software\RevoProject\RevoStream" Name="shortcut"
                       Type="integer" Value="1" KeyPath="yes"/>
      </Component>
    </DirectoryRef>
    <Feature Id="MainFeature" Title="RevoStream" Level="1">
      <ComponentGroupRef Id="Payload"/>
      <ComponentRef Id="ApplicationShortcut"/>
    </Feature>
  </Product>
</Wix>
"@ | Set-Content -Encoding utf8 $Product

    $ObjectDir = Join-Path $BundleDir "wix-objects"
    New-Item -ItemType Directory -Force -Path $ObjectDir | Out-Null
    $Msi = Join-Path $BundleDir "$Prefix.msi"
    if (Test-Path $Msi) { Remove-Item -Force $Msi }
    & $Candle -nologo -arch x64 "-dPayloadSource=$Stage" -out $ObjectDir $Product $Fragment
    if ($LASTEXITCODE -ne 0) { throw "candle failed with $LASTEXITCODE" }
    & $Light -nologo -ext WixUIExtension -out $Msi (Get-ChildItem $ObjectDir -Filter *.wixobj | ForEach-Object FullName)
    if ($LASTEXITCODE -ne 0) { throw "light failed with $LASTEXITCODE" }
    if (-not (Test-Path $Msi)) { throw "MSI was not produced" }
    Write-Host "    $Msi"
}

Write-Host "=== Done ==="
Get-ChildItem $BundleDir -File | Where-Object Name -like "revo-gpui-$Version-*" |
    ForEach-Object { Write-Host ("  {0,-46} {1,10:N0} bytes" -f $_.Name, $_.Length) }
