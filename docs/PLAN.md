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
