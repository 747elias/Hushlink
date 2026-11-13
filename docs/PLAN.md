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
