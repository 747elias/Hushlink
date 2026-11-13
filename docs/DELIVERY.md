# Windows build and release delivery

## Native executable

Shipping target: Windows 11 x64, `x86_64-pc-windows-msvc`. The pinned Rust toolchain is declared in `rust-toolchain.toml`. `.cargo/config.toml` enables the static MSVC runtime. The native Win32 Bluetooth/Winsock API does not need a Store/MSIX package identity, a capability declaration, a service installation or an administrator elevation. The supplied application manifest requests ordinary user privileges and per-monitor DPI awareness.

The EXE is a Windows GUI application with an original embedded ICO and Windows version resources. The icon's editable generator is `scripts/generate-icon.py` (Pillow is needed only to regenerate the image). The normal build does not need Python or Pillow installed on the end user's PC. The UI uses Windows-provided graphics interfaces and requires a functioning desktop graphics driver.

No code-signing credentials are configured. An unsigned release must never be described as trusted-publisher signed; Windows may show SmartScreen warnings. Do not disable protection globally to use it.

## Automated checks

- `CI` runs on branch pushes and pull requests. A separate path-filtered `RFCOMM technical spike` workflow compiles the small transport probe independently of unfinished UI code and unrelated source pushes.
- A reusable `Required checks` workflow runs on a current Windows 2025 GitHub-hosted runner with Rust 1.91.1 and the native x64 MSVC target.
- The isolated `tools/rfcomm-probe` is linked before UI/integration checks, so failures in unrelated UI code do not conceal Windows Bluetooth API errors. Compiling/linking this probe is not a hardware test.
- The full checks require formatting, strict Clippy, locked dependency tests, a locked native release build and a native startup smoke test. The EXE must remain running for eight seconds on the Windows runner. A screenshot is captured where desktop APIs allow and uploaded with startup metadata. This does not establish interactive UI correctness or Bluetooth access; inspect the screenshot and record any desktop limitation separately.
- `scripts/check-pe.py` verifies that the output is an x64 PE32+ Windows GUI executable. The executable and the exact `github.sha` build marker become a same-run artifact.

All third-party Actions are pinned to complete commit SHA values, resolved from their official release refs. Rust package direct versions are exact; Cargo.lock fixes the full dependency graph. Workflow permissions default to `contents: read`. No pull-request workflow receives privileged write permissions. Standard GitHub-hosted runner tools, Windows SDK and operating-system components follow the maintained runner image; the build does not claim bit-for-bit reproducibility.

## Release gate

Pushing a `v*` tag starts the `Release` workflow. The tag's exact commit reruns all reusable checks and builds the EXE. A successful branch run does not bypass or replace these tag checks. Packaging depends on that successful job and validates that the artifact's commit marker equals `github.sha` and the tag version equals Cargo.toml.

`scripts/package.ps1` creates the versioned standalone EXE, ZIP (including EXE, quickstart, documentation and license), and SHA256SUMS.txt. It re-reads both assets, verifies their hashes, extracts the ZIP and checks that its EXE is byte-identical to the standalone download. A separate publication job downloads these checked assets and verifies SHA-256 again. Only that job has `contents: write`; it creates a new prerelease without overwriting an existing release.

The gate proves automated checks for a particular commit and the origin/structure of the generated artifact. It proves neither two-device Bluetooth behavior nor an external security audit. Those limits must remain in the release notes.

After publication, independently download all assets, compare SHA-256, inspect PE format, and verify the release tag resolves to the checked commit. Record the run URL and release URL. Never claim publication based only on a pushed tag.
