import os
from pathlib import Path
obs = (
    Path(os.environ["RUNNER_TEMP"])
    / "obs-studio"
)
path = (
    obs
    / "libobs-metal/CMakeLists.txt"
)
if not path.exists():
    raise SystemExit(
        f"Missing OBS Metal CMakeLists: {path}"
    )
text = path.read_text()
if "enable_language(Swift)" in text:
    print(
        "OBS already enables Swift"
    )
else:
    path.write_text(
        "enable_language(Swift)\n\n"
        + text
    )
    print(
        "Added enable_language(Swift)"
    )
