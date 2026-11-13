"""Check that a built/downloaded file is an x64 Windows GUI executable.

This validates the container, not hardware compatibility or code safety.
"""
from pathlib import Path
import struct
import sys

path = Path(sys.argv[1])
with path.open("rb") as handle:
    header = handle.read(64)
    if len(header) != 64 or header[:2] != b"MZ":
        raise SystemExit("Not a Windows PE executable (MZ missing)")
    pe_offset = struct.unpack_from("<I", header, 0x3C)[0]
    handle.seek(pe_offset)
    pe = handle.read(94)
    if len(pe) != 94 or pe[:4] != b"PE\0\0":
        raise SystemExit("Not a Windows PE executable (PE signature missing)")
    machine = struct.unpack_from("<H", pe, 4)[0]
    magic = struct.unpack_from("<H", pe, 24)[0]
    subsystem = struct.unpack_from("<H", pe, 24 + 68)[0]
    if machine != 0x8664 or magic != 0x20B or subsystem != 2:
        raise SystemExit(f"Expected x64 PE32+ Windows GUI; got {machine:#x}, {magic:#x}, {subsystem}")
print(f"Verified x64 PE32+ Windows GUI executable: {path.name} ({path.stat().st_size} bytes)")
