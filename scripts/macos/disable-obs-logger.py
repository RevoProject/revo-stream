from pathlib import Path
import re

linux_cfg = '#[cfg(target_os = "linux")]'

# logging/mod.rs
mod = Path("src-tauri/src/logging/mod.rs")
text = mod.read_text()
text = re.sub(
    r'(?m)^[ \t]*#\[cfg\([^\n]+\)\]\n([ \t]*pub mod obs_logger;)',
    linux_cfg + r'\n\1',
    text,
    count=1,
)
if linux_cfg + "\npub mod obs_logger;" not in text:
    raise SystemExit("ERROR: obs_logger module was not made Linux-only")
mod.write_text(text)

# logging/obs_logger.rs
logger = Path("src-tauri/src/logging/obs_logger.rs")
if logger.exists():
    logger_text = logger.read_text()
    logger_text = re.sub(
        r'\A#!\[cfg\([^\n]+\)\]\n?',
        linux_cfg.replace('#[', '#![') + "\n",
        logger_text,
        count=1,
    )
    if not logger_text.startswith('#![cfg(target_os = "linux")]'):
        logger_text = '#![cfg(target_os = "linux")]\n\n' + logger_text
    logger.write_text(logger_text)

# runtime/init.rs
init = Path("src-tauri/src/runtime/init.rs")
init_text = init.read_text()
marker = "crate::logging::obs_logger::obs_log_handler"
statement = "revo_lib::obs::base_set_log_handler("

if marker not in init_text:
    raise SystemExit("ERROR: obs_log_handler reference not found in runtime/init.rs")

handler_pos = init_text.find(marker)
statement_start = init_text.rfind(statement, 0, handler_pos)

if statement_start == -1:
    raise SystemExit("ERROR: base_set_log_handler call not found")

line_start = init_text.rfind("\n", 0, statement_start) + 1
statement_prefix = init_text[line_start:statement_start]

if statement_prefix.strip():
    raise SystemExit("ERROR: unexpected content before base_set_log_handler")

indent = statement_prefix
prefix = init_text[:line_start]
prefix_lines = prefix.splitlines(keepends=True)

while prefix_lines and not prefix_lines[-1].strip():
    prefix_lines.pop()

while prefix_lines and re.fullmatch(
    r'[ \t]*#\[cfg\([^\n]+\)\][ \t]*',
    prefix_lines[-1].rstrip("\r\n")
):
    prefix_lines.pop()
    while prefix_lines and not prefix_lines[-1].strip():
        prefix_lines.pop()

prefix = "".join(prefix_lines)
if prefix and not prefix.endswith("\n"):
    prefix += "\n"

init_text = (
    prefix
    + f'{indent}#[cfg(target_os = "linux")]\n'
    + init_text[line_start:]
)
init.write_text(init_text)

# Verify the exact call that references obs_log_handler.
init_text = init.read_text()
handler_pos = init_text.find(marker)
statement_start = init_text.rfind(statement, 0, handler_pos)

if statement_start == -1:
    raise SystemExit("ERROR: base_set_log_handler call disappeared")

line_start = init_text.rfind("\n", 0, statement_start) + 1
before_call = init_text[:line_start].splitlines()

while before_call and not before_call[-1].strip():
    before_call.pop()

if (
    not before_call
    or before_call[-1].strip()
    != '#[cfg(target_os = "linux")]'
):
    raise SystemExit(
        "ERROR: exact OBS logger call is not Linux-only"
    )

print("OBS logger is Linux-only")
