# rv_edit4 diagnostic handover

## Scope

The `rv_edit4` archive contains source diagnostics and procedures, not an
installer or a release. The application version remains `0.4.1`.

Archive manifest verification performed during integration:

- 39 manifest entries checked
- 0 missing files
- 0 SHA-256 mismatches
- patch 16: `FAA7A1D03244868BEC9598FC0C8C3584DE4C13866F54AF1BC90C34DDFB684297`
- patch 17: `C95CB81DB9B7960597879154F7282EA3D4043A6FB72C2ADDC4C43737D4A55979`

Patches 01-15 were integrated previously. Patches 16 and 17 add diagnostics
only. They do not modify transitions, source visibility, capture frequency,
CDP deadlines or cancellation behavior.

## Not confirmed fixed

- Frozen preview during scene transitions is not confirmed fixed.
- Runtime mutex starvation is not confirmed. The probe only measures it.
- Browser Interaction CPU usage has not been measured on Windows.
- Display Capture DPI behavior still requires the exact Windows version and
  scaling configuration.
- Correct recording during a preview freeze and preview recovery are separate
  facts and must be reported separately.

Do not add watchdogs, terminate OBS, change capture behavior or introduce
arbitrary timeouts until a Windows measurement identifies the blocking stage.
Do not publish `0.4.1` as a stability release based only on these diagnostics.

## DevTools diagnostics

`withGlobalTauri` is disabled. `window.__TAURI__` being undefined is expected.
Use the read-only bridge exposed by the application:

```js
window.__revoPreviewStats
await window.__revoPreviewDiag()
await window.__revoInvoke("debug_trace_last")
await window.__revoInvoke("debug_interact_stats")
```

`debug_preview_stats` and `debug_interact_stats` use independent counters and
`try_lock`. They report `runtime_lock: "busy"` or `sessions_lock: "busy"`
instead of blocking behind the operation being investigated.

The absence of `lock-wait` or `lock-hold` log lines does not prove there is no
contention. Warnings are rate limited. Use `mutex_probe.ops` counters:

- high wait with consistently short holds: contention/starvation candidate
- short waits with one large max hold: the named operation is the bottleneck
- `currently_held`: operation holding a probed mutex at snapshot time

## Preview freeze procedure

1. Start recording so output correctness can be checked independently.
2. Switch A -> B -> A -> B at roughly one-second intervals with a transition.
3. Wait ten seconds after the preview appears frozen.
4. Capture `__revoPreviewDiag`, `debug_trace_last` and logs.
5. Repeat without a transition, with a stinger, and with Browser Interaction.
6. Inspect the recording separately from whether preview recovered.

Interpretation:

- `frozen_returns` grows while `captures_ok` does not: freeze gate remains set
- `captures_err` and `stale_returns` grow: render fails and retained frame is served
- backend `captures_ok` grows but frontend `displayed` does not: frontend path
- `runtime_lock: "busy"`: use mutex probe to identify wait versus hold
- old `trace.age_ms` with no progress: application-level stall candidate

## Browser Interaction measurements

Run separate 60-second scenarios with application restart between them:

1. interaction closed
2. blank page
3. static page
4. animated page

Keep URL, User-Agent, CSS, JS and viewport identical between comparable runs;
changing any of them intentionally respawns Chromium. Record CPU, memory,
Chromium process count, screencast frames/bytes, forced captures and mutex
wait/hold counters.

Session reap is triggered by `ensure_session`; it is not a background timer.
The close path may take up to the CDP deadline, so an immediate process check
can report a false leak.

## Windows report requirements

The return report must include:

- exact reproduction steps and Windows build from `winver`
- display scaling and GPU/driver details
- recording correctness during the freeze
- preview recovery as a separate result
- last trace step and age
- frontend/backend preview counters
- mutex wait/hold snapshot
- CPU, RAM, disk and page-file measurements from the start of the run
- Browser Interaction windows/processes and close latency
- hypotheses with evidence that confirms or rules each one out
- changed files and resulting commit SHA

Compilation and static tests prove only that the diagnostics build. They are
not Windows runtime evidence.
