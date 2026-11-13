@echo off
setlocal
set "APPDIR=%~dp0"
set "REVO_ROOT=%APPDIR%revo-root"
rem revo-engine.exe lives inside the runtime directory in this package (not next to the UI).
if not defined REVO_ENGINE_BIN set "REVO_ENGINE_BIN=%APPDIR%revo-root\core\lib\revo-engine.exe"
if not defined REVO_DATA_DIR set "REVO_DATA_DIR=%LOCALAPPDATA%\com.klubuntuprojects.revo"
set "PATH=%APPDIR%revo-root\core\lib;%PATH%"
if not exist "%REVO_ENGINE_BIN%" echo [revo] WARNING: engine binary not found: "%REVO_ENGINE_BIN%" - OBS runtime will not start.
rem --obs is required: without it the UI runs the built-in mock backend and never spawns revo-engine.
start "" "%APPDIR%revo-gpui.exe" --obs %*
