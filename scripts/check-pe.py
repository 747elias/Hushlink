"""Check that a built/downloaded file is an x64 Windows GUI executable.

This validates the container, not hardware compatibility or code safety.
"""
import argparse
from pathlib import Path
import struct

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("path", type=Path)
parser.add_argument("--installer", action="store_true", help="allow a GUI x86 installer bootstrap as well as x64")
args = parser.parse_args()
path = args.path
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
    allowed = {(0x8664, 0x20B)}
    if args.installer:
        allowed.add((0x14C, 0x10B))
    if (machine, magic) not in allowed or subsystem != 2:
        raise SystemExit(f"Expected x64 PE32+ Windows GUI; got {machine:#x}, {magic:#x}, {subsystem}")
kind = "x64 PE32+" if machine == 0x8664 else "x86 PE32 installer bootstrap"
print(f"Verified {kind} Windows GUI executable: {path.name} ({path.stat().st_size} bytes)")
