import os
from pathlib import Path
obs = (
    Path(os.environ["RUNNER_TEMP"])
    / "obs-studio"
)
path = (
    obs
    / "plugins/mac-virtualcam/src/dal-plugin/OBSDALStream.mm"
)
if not path.exists():
    raise SystemExit(
        f"Missing OBSDALStream.mm: {path}"
    )
text = path.read_text()
pragma = (
    '#pragma clang diagnostic push\n'
    '#pragma clang diagnostic ignored "-Wimplicit-int-float-conversion"\n'
    '#pragma clang diagnostic ignored "-Wenum-enum-conversion"\n'
)
if (
    "#pragma clang diagnostic push"
    not in text
):
    text = pragma + "\n" + text
if (
    "#pragma clang diagnostic pop"
    not in text
):
    text = (
        text.rstrip()
        + "\n\n#pragma clang diagnostic pop\n"
    )
path.write_text(text)
print(f"Patched: {path}")
