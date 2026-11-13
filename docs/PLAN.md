# Hushlink v0.1.0 implementation plan

1. Inspect repository, permissions and environment; validate a Windows Classic RFCOMM listener, service registration and client path against official documentation and a Windows compilation.
2. Implement and test Noise sessions, mutual trust and bounded framing; implement authenticated encrypted vault and atomic persistence.
3. Integrate real Bluetooth workers with a German egui desktop UI, lock lifecycle and complete one-to-one messaging.
4. Run formatting, strict lint, tests and Windows MSVC builds; fix failures and document physical two-PC checks separately.
5. Publish a gated GitHub prerelease, download EXE/ZIP/checksums and verify commit, SHA-256 and PE format.

## Initial evidence

- Existing empty task repository: https://github.com/747elias/Hushlink, private, branch `work`, origin preserved.
- Authenticated GitHub account has ADMIN access; no existing commits, files or changes.
- Execution host: Linux x86_64; no local Windows/Bluetooth hardware. Windows builds must use GitHub Actions.
- Installing a pinned Rust toolchain locally in ignored work/.

## Integration progress

- Native Classic RFCOMM listener, own SDP service, accept path and UUID-based connector implemented. Stable UUID `7d2ea28a-f7bd-4a6d-b1f4-2cf2f653cc0e`. Native Win32 Winsock avoids package identity; static CRT Windows EXE planned.
- Isolated native MSVC build/link and transport parser tests passed on Windows2025: https://github.com/747elias/Hushlink/actions/runs/43435018711 (commit `56c0fd52a9aedebbd8037e2a80d913582902eb88`). No physical radio test.
- Protocol module: 15 targeted tests passed. Vault module: 8 targeted tests passed. Integration actor and full UI validation in progress.
- GitHub private-email restriction resolved with a new `feature/first-release` branch using GitHub noreply commits; original unpublished local `work` branch preserved, no history rewrite or force push. Incremental pushes verified.
- Internal dependency audit corrected two unsoundness advisories by updating anyhow/rand. Remaining `ttf-parser` maintenance warning documented in SECURITY.md.
- Native Windows startup smoke and screenshot capture added to required checks. Cloud desktop control tool failed to initialize (`gaas-browser node_repl sandbox cannot be disabled`); investigating direct local X11 testing.
- Remaining: full integration checks, UI inspection/fixes, release-tag checks, prerelease and independent downloads verification.

## Release candidate validated

- Application modules and requested one-to-one workflows implemented. No production test transport or IP fallback.
- Native WTS session notifications and background inactivity locking close sessions without relying on UI frames. UI text/undo/input buffers are cleared, and Windows Light mode cannot override the dark UI.
- Linux: formatting, strict Clippy and 38 tests passed. Real X11 first-start/unlock/password-change/lock/small-window checks passed with synthetic profiles only.
- Windows: run 43496556227 for `8b283ccc32543c8a53330cea109717ecbaaad95d` passed all required checks, 39 tests, native MSVC release build, eight-second native startup, x64 GUI PE verification, complete EXE/ZIP/SHA-256 prospective packaging.
- Native startup defects (OpenGL driver limitation, missing explicit DX12 backend), Windows mandatory-lock test assumptions and CRLF version parsing were corrected and pushed; no checks were skipped.
- Final step: publish v0.1.0 through the gated tag workflow, rerun exact-commit checks and independently verify the actual published downloads. The workflow records post-publication verification evidence.
- Physical two-PC Bluetooth and actual Win+L/Windows scaling acceptance remain unperformed; docs/MANUAL-TEST.md is the concrete checklist.
