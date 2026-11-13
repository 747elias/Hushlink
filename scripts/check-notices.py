"""Refuse release packaging when the checked-in notices describe another lockfile."""
import hashlib
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
notices = (root / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf-8")
recorded = re.search(r"Cargo\.lock SHA-256: `([0-9a-f]{64})`", notices)
actual = hashlib.sha256((root / "Cargo.lock").read_bytes().replace(b"\r\n", b"\n")).hexdigest()
if not recorded or recorded[1] != actual:
    raise SystemExit("Dependency notices are stale. Run python scripts/generate-notices.py with the pinned toolchain and commit the updated notices.")
for required in ("OFL", "UFL", "Hack-Regular", "emoji-icon-font"):
    if required not in notices:
        raise SystemExit(f"Embedded font notice missing: {required}")
print("Dependency notices match Cargo.lock; embedded font notices are present.")
