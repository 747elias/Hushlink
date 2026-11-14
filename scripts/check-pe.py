"""Check that a built/downloaded file is an x64 Windows GUI executable.

This validates the container, not hardware compatibility or code safety.
"""
import argparse
from pathlib import Path
import struct

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("path", type=Path)
parser.add_argument("--installer", action="store_true", help="allow a GUI x86 installer bootstrap as well as x64")
parser.add_argument("--optional-camera", action="store_true", help="require Media Foundation to be delay-loaded, never a startup dependency")
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

if args.optional_camera:
    data = path.read_bytes()
    section_count = struct.unpack_from("<H", data, pe_offset + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe_offset + 20)[0]
    optional = pe_offset + 24
    directories = optional + (112 if magic == 0x20B else 96)
    sections = optional + optional_size
    if not 1 <= section_count <= 96 or sections + 40 * section_count > len(data):
        raise SystemExit("Invalid PE sections")

    def offset(rva):
        for index in range(section_count):
            entry = sections + 40 * index
            virtual_size, address, raw_size, raw_start = struct.unpack_from("<4I", data, entry + 8)
            if address <= rva < address + max(virtual_size, raw_size):
                relative = rva - address
                if relative >= raw_size or raw_start + relative >= len(data):
                    raise SystemExit("PE import points outside file bytes")
                return raw_start + relative
        raise SystemExit("Unmapped PE import RVA")

    def imported(index, delayed):
        rva, length = struct.unpack_from("<2I", data, directories + 8 * index)
        if not rva:
            return set()
        start = offset(rva)
        stride = 32 if delayed else 20
        names = set()
        for number in range(512):
            entry = start + number * stride
            if entry + stride > len(data) or number * stride >= length:
                raise SystemExit("Unterminated PE import descriptors")
            descriptor = data[entry:entry + stride]
            if not any(descriptor):
                return names
            if delayed and struct.unpack_from("<I", descriptor)[0] != 1:
                raise SystemExit("Unexpected delay-import addressing")
            name_rva = struct.unpack_from("<I", descriptor, 4 if delayed else 12)[0]
            begin = offset(name_rva)
            end = data.find(b"\0", begin, min(begin + 256, len(data)))
            if end < 0:
                raise SystemExit("Invalid PE import name")
            names.add(data[begin:end].decode("ascii").lower())
        raise SystemExit("Too many PE imports")

    regular, delayed = imported(1, False), imported(13, True)
    media = {"mf.dll", "mfplat.dll", "mfreadwrite.dll"}
    if media & regular or not media.issubset(delayed):
        raise SystemExit("Camera libraries must be delay-loaded to preserve startup without Media Foundation")
    if any(name.startswith(("vcruntime", "msvcp", "msvcr")) for name in regular | delayed):
        raise SystemExit("Unexpected external Visual C++ runtime dependency")
    print("Verified optional camera DLLs are delay-loaded; no separate VC++ runtime")
