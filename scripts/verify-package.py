"""Verify local release assets before publishing the exact checked bytes."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys
import zipfile


def sha256(path):
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def verify(directory, tag, commit, require_exact_files=True):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?", tag):
        raise SystemExit("Invalid version tag")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise SystemExit("Expected full lowercase commit SHA")
    base = f"Hushlink-{tag}-windows-x64"
    payload_names = {base + ".exe", base + "-setup.exe", base + ".zip"}
    checksums = {}
    for line in (directory / "SHA256SUMS.txt").read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9_.-]+)", line)
        if not match or match[2] in checksums:
            raise SystemExit("Invalid or duplicate SHA256SUMS entry")
        checksums[match[2]] = match[1]
    if set(checksums) != payload_names:
        raise SystemExit("Unexpected checksum asset names")
    if require_exact_files and {path.name for path in directory.iterdir() if path.is_file()} != payload_names | {"SHA256SUMS.txt"}:
        raise SystemExit("Unexpected or missing release files")
    for name, expected in checksums.items():
        if sha256(directory / name) != expected:
            raise SystemExit(f"SHA-256 mismatch: {name}")
    pe_checker = str(Path(__file__).with_name("check-pe.py"))
    subprocess.run([sys.executable, pe_checker, str(directory / (base + ".exe"))], check=True)
    subprocess.run([sys.executable, pe_checker, str(directory / (base + "-setup.exe")), "--installer"], check=True)
    with zipfile.ZipFile(directory / (base + ".zip")) as archive:
        # Avoid ambiguous archives even though only known members are read here.
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise SystemExit("ZIP has duplicate entry names")
        for name in (base + ".exe", base + "-setup.exe"):
            with archive.open(name) as handle:
                value = hashlib.sha256()
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    value.update(chunk)
            if value.hexdigest() != checksums[name]:
                raise SystemExit(f"ZIP does not contain the identical checked binary: {name}")
        if archive.read("BUILD-COMMIT.txt").decode("utf-8").strip() != commit:
            raise SystemExit("ZIP build commit differs from the checked commit")
        if not {"KURZSTART.md", "LICENSE", "RELEASE-NOTES.md"}.issubset(names):
            raise SystemExit("ZIP is missing required documentation")
    return checksums


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    for name, digest in sorted(verify(args.directory, args.tag, args.commit).items()):
        print(f"{digest}  {name}")
