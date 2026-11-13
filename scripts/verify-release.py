"""Independently download and verify an already-published Hushlink prerelease.

Uses the authenticated gh CLI without exposing credentials. Outputs only public
release/commit/asset metadata. Downloaded files stay outside the source tree by
using an ignored work/ directory. This does not test Bluetooth hardware.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import zipfile


def gh_json(*args):
    return json.loads(subprocess.check_output(["gh", *args], text=True))


def sha256(path):
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--repo", required=True)
parser.add_argument("--tag", required=True)
parser.add_argument("--commit", required=True)
parser.add_argument("--directory", type=Path, required=True)
args = parser.parse_args()
if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repo):
    raise SystemExit("Invalid repository name")
if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?", args.tag):
    raise SystemExit("Invalid version tag")
if not re.fullmatch(r"[0-9a-f]{40}", args.commit):
    raise SystemExit("Expected full lowercase commit SHA")
ref = gh_json("api", f"repos/{args.repo}/git/ref/tags/{args.tag}")
obj = ref["object"]
for _ in range(8):
    if obj["type"] != "tag":
        break
    obj = gh_json("api", f"repos/{args.repo}/git/tags/{obj['sha']}")["object"]
if obj["type"] != "commit" or obj["sha"] != args.commit:
    raise SystemExit("Release tag does not resolve to the expected checked commit")
release = gh_json("api", f"repos/{args.repo}/releases/tags/{args.tag}")
if release["draft"] or not release["prerelease"] or release["tag_name"] != args.tag:
    raise SystemExit("Expected a published prerelease for the requested tag")
base = f"Hushlink-{args.tag}-windows-x64"
expected_names = {base + ".exe", base + ".zip", "SHA256SUMS.txt"}
assets = {asset["name"]: asset for asset in release["assets"]}
if not expected_names.issubset(assets):
    raise SystemExit("Published prerelease is missing required assets")
args.directory.mkdir(parents=True, exist_ok=True)
for name in sorted(expected_names):
    subprocess.run(["gh", "release", "download", args.tag, "--repo", args.repo,
                    "--pattern", name, "--dir", str(args.directory), "--clobber"], check=True)
    if (args.directory / name).stat().st_size != assets[name]["size"]:
        raise SystemExit(f"Downloaded size differs from release metadata: {name}")
checksums = {}
for line in (args.directory / "SHA256SUMS.txt").read_text(encoding="utf-8").splitlines():
    match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9_.-]+)", line)
    if not match or match[2] in checksums:
        raise SystemExit("Invalid or duplicate SHA256SUMS entry")
    checksums[match[2]] = match[1]
if set(checksums) != {base + ".exe", base + ".zip"}:
    raise SystemExit("Unexpected checksum asset names")
for name, expected in checksums.items():
    if sha256(args.directory / name) != expected:
        raise SystemExit(f"SHA-256 mismatch: {name}")
subprocess.run([sys.executable, str(Path(__file__).with_name("check-pe.py")),
                str(args.directory / (base + ".exe"))], check=True)
with zipfile.ZipFile(args.directory / (base + ".zip")) as archive:
    if hashlib.sha256(archive.read(base + ".exe")).hexdigest() != checksums[base + ".exe"]:
        raise SystemExit("ZIP does not contain the identical checked EXE")
    if archive.read("BUILD-COMMIT.txt").decode("utf-8").strip() != args.commit:
        raise SystemExit("ZIP build commit differs from the checked commit")
evidence = {"repository": args.repo, "tag": args.tag, "commit": args.commit,
            "release_url": release["html_url"], "release_id": release["id"],
            "prerelease": release["prerelease"], "sha256": checksums,
            "download_urls": {name: assets[name]["browser_download_url"] for name in sorted(expected_names)},
            "verification": "Downloaded assets, sizes, SHA-256, tag commit, x64 GUI PE and ZIP EXE identity verified; no Bluetooth hardware test."}
(args.directory / "verification.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
print(json.dumps(evidence, indent=2))
