"""Collect actual license texts for the locked Windows normal/build dependency graph.

Uses crate-shipped texts. For archives that omit a license, fetch an official
upstream license at the archive's recorded VCS revision and retain it locally.
No dependency code or personal files are copied. Run from repository root with
its pinned Rust toolchain and Python 3.11+ available.
"""
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-pc-windows-msvc"
UPSTREAM = ROOT / "licenses/upstream"
UPSTREAM.mkdir(parents=True, exist_ok=True)


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True)


def font_copyright(path):
    data = path.read_bytes()
    for i in range(struct.unpack_from(">H", data, 4)[0]):
        entry = 12 + i * 16
        if data[entry:entry+4] != b"name":
            continue
        start = struct.unpack_from(">I", data, entry + 8)[0]
        count, strings = struct.unpack_from(">HH", data, start + 2)
        results = set()
        for j in range(count):
            platform, _, _, name_id, length, offset = struct.unpack_from(">6H", data, start + 6 + j * 12)
            if name_id != 0:
                continue
            raw = data[start+strings+offset:start+strings+offset+length]
            encoding = "utf-16-be" if platform in (0, 3) else "mac_roman"
            results.add(raw.decode(encoding).strip())
        return "\n".join(sorted(results))
    return ""


meta = json.loads(command("cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", TARGET))
tree = command("cargo", "tree", "--locked", "--target", TARGET, "--edges", "normal,build", "--prefix", "none", "--format", "{p}")
selected = set()
for line in tree.splitlines():
    match = re.match(r"([\w-]+) v([^ ]+)", line)
    if match:
        selected.add(match.groups())
packages = [p for p in meta["packages"] if (p["name"], p["version"]) in selected and p["source"]]
license_texts = {}
rows = []
provenance = {}
font_notices = []
for package in sorted(packages, key=lambda p: (p["name"], p["version"])):
    folder = Path(package["manifest_path"]).parent
    files = [p for p in folder.iterdir() if p.is_file() and p.name.lower().startswith(("license", "copying", "notice"))]
    sources = [(path.relative_to(folder).as_posix(), path.read_text(encoding="utf-8", errors="replace")) for path in sorted(files)]
    if not sources:
        vcs = json.loads((folder / ".cargo_vcs_info.json").read_text())
        revision = vcs["git"]["sha1"]
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            raise SystemExit("Invalid upstream revision in crate archive")
        name = package["name"]
        if name in {"ecolor", "eframe", "egui", "egui-winit", "egui_glow", "emath", "epaint", "epaint_default_fonts"}:
            repo, filename = "emilk/egui", "LICENSE-MIT"
        elif name in {"gl_generator", "khronos_api"}:
            repo, filename = "brendanzab/gl-rs", "LICENSE"
        elif name == "clipboard-win":
            repo, filename = "DoumanAsh/clipboard-win", "LICENSE"
        elif name == "profiling":
            repo, filename = "aclysma/profiling", "LICENSE-MIT"
        else:
            raise SystemExit(f"Missing actual license text for {name} {package['version']}")
        url = f"https://raw.githubusercontent.com/{repo}/{revision}/{filename}"
        retained = UPSTREAM / f"{repo.split('/')[1]}-{revision[:12]}-{filename}.txt"
        if not retained.exists():
            subprocess.run(["curl", "--fail", "--silent", "--show-error", "--location", url, "--output", str(retained)], check=True)
        provenance[retained.relative_to(ROOT).as_posix()] = url
        sources.append((url, retained.read_text(encoding="utf-8")))
    if package["name"] == "epaint_default_fonts":
        for path in sorted((folder / "fonts").glob("*.txt")):
            sources.append((path.relative_to(folder).as_posix(), path.read_text(encoding="utf-8")))
        for path in sorted((folder / "fonts").glob("*.ttf")):
            font_notices.append((path.name, font_copyright(path)))
    refs = []
    for source, text in sources:
        text = text.replace("\r\n", "\n").strip() + "\n"
        digest = hashlib.sha256(text.encode()).hexdigest()
        if digest not in license_texts:
            license_texts[digest] = (f"L{len(license_texts)+1:03}", text)
        label = license_texts[digest][0]
        refs.append(f"[{label}](#{label.lower()}) ({source})")
    license_name = package["license"] or "See actual license text"
    rows.append(f"| {package['name']} {package['version']} | {license_name.replace('|', '/')} | {'; '.join(refs)} |")
lock_sha = hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()
parts = ["# Third-party notices\n", "This file preserves license/copyright texts for the locked Windows normal/build dependency graph, including build-only crates. Test-only dependencies are excluded. For a crate offered under MIT OR Apache-2.0, the MIT option is used where available; additional shipped notices are retained for completeness. Font licenses apply to the embedded font software. Hushlink's own license is in LICENSE.\n", f"Target: `{TARGET}`. Cargo.lock SHA-256: `{lock_sha}`.\n", "Generated with `python scripts/generate-notices.py`; licenses missing from crate archives are retained from the exact upstream VCS revision recorded in the archive. See [source provenance](licenses/upstream-sources.json).\n", "The pinned Rust toolchain's [standard-library notices](licenses/rust-std/COPYRIGHT-library.html) and license texts are included separately.\n", "| Dependency | Declared license | Actual license texts |\n| --- | --- | --- |", *rows, "\n## Embedded font copyright notices\n"]
for name, text in font_notices:
    parts.extend([f"### {name}\n", "```text\n" + text + "\n```\n"])
for _, (label, text) in license_texts.items():
    parts.extend([f"## {label}\n", "```text\n" + text + "```\n"])
(ROOT / "THIRD_PARTY_NOTICES.md").write_text("\n".join(parts), encoding="utf-8")
(ROOT / "licenses/upstream-sources.json").write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
sysroot = Path(command("rustc", "--print", "sysroot").strip())
rust_docs = sysroot / "share/doc/rust"
rust_target = ROOT / "licenses/rust-std"
rust_target.mkdir(parents=True, exist_ok=True)
shutil.copyfile(rust_docs / "COPYRIGHT-library.html", rust_target / "COPYRIGHT-library.html")
shutil.copytree(rust_docs / "licenses", rust_target / "licenses", dirs_exist_ok=True)
print(f"Collected {len(packages)} dependency notices, {len(license_texts)} distinct license texts and {len(font_notices)} embedded font notices.")
