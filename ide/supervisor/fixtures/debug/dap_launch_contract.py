"""Bounded fixed-launch diagnostic helpers; no execution or authorization."""
import json

MAX_HEADER = 512
MAX_BODY = 32768
MAX_BUFFER = MAX_HEADER + 4 + MAX_BODY
FIXED_TARGETS = {
    "/opt/lu-ide-wo6-dap-test/debuggee/wo6-selftest",
    "/ls/debuggee/wo6-selftest",
}


def launch_arguments(target):
    if target not in FIXED_TARGETS:
        raise ValueError("invalid_fixed_target")
    return {
        "program": target, "stopOnEntry": True, "noDebug": False,
        "disableASLR": False, "disableSTDIO": True,
        "args": [], "env": {}, "runInTerminal": False,
    }


def pop_frame(buffer):
    """Consume exactly one complete JSON object; retain an incomplete frame."""
    if len(buffer) > MAX_BUFFER:
        raise ValueError("invalid_frame")
    header_end = buffer.find(b"\r\n\r\n")
    if header_end < 0:
        if len(buffer) > MAX_HEADER + 3:
            raise ValueError("invalid_frame")
        return None
    header = bytes(buffer[:header_end])
    prefix = b"Content-Length: "
    if header_end > MAX_HEADER or not header.startswith(prefix):
        raise ValueError("invalid_frame")
    size_text = header[len(prefix):]
    if not size_text.isdigit() or len(size_text) > 5:
        raise ValueError("invalid_frame")
    size = int(size_text)
    if not 0 < size <= MAX_BODY:
        raise ValueError("invalid_frame")
    end = header_end + 4 + size
    if len(buffer) < end:
        return None
    try:
        item = json.loads(buffer[header_end + 4:end])
    except (ValueError, UnicodeError, RecursionError):
        raise ValueError("invalid_frame") from None
    if not isinstance(item, dict):
        raise ValueError("invalid_frame")
    del buffer[:end]
    return item
