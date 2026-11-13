RevoStream (native GPUI build) 0.5.0-pre1

revo-gpui.exe    UI process (GPUI renderer, no libobs in this process; mock backend by default)
revo-engine.exe  libobs runtime, spawned automatically by the UI - located at
                 revo-root\core\lib\revo-engine.exe in this package
revo-root\       OBS runtime: core\lib, core\bin, core\share, data
launch.cmd       REQUIRED entry point. It puts revo-root\core\lib on PATH so the
                 engine's import of obs.dll resolves, exports REVO_ROOT and
                 REVO_ENGINE_BIN, and starts the UI with the --obs flag.

IMPORTANT
  revo-gpui.exe started on its own runs the built-in MOCK backend (demo scenes,
  no OBS, no revo-engine process). The real libobs engine is only used when the
  UI is started with --obs. launch.cmd already passes it.

  Environment variables (set by launch.cmd, can be overridden):
    REVO_ROOT        relocate the runtime (default: <app>\revo-root)
    REVO_ENGINE_BIN  path to revo-engine.exe
                     (default: <app>\revo-root\core\lib\revo-engine.exe)
    REVO_ENGINE_SOCK attach to an already running engine instead of spawning it
                     (endpoint string, e.g. tcp:127.0.0.1:5599)
    REVO_DATA_DIR    per-user settings and logs
                     (default: %LOCALAPPDATA%\com.klubuntuprojects.revo)

  Logs:      %REVO_DATA_DIR%\logs\<year>\*.log
             No logs directory means the engine never started.
  Diagnose:  press F10 in the main window to open the built-in Debug Console.
