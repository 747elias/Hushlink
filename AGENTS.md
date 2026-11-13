# Hushlink contributor rules

- Keep Rust UI, transport, session protocol and encrypted storage separate. Windows 11 x64 MSVC is the shipping target. Never substitute TCP/WLAN for Bluetooth.
- Never commit credentials, identities, local vaults, personal messages, target/ or temporary assets. Tests use synthetic data only. Never log plaintext, passwords or secret keys.
- Preserve unrelated work. No force pushes, destructive resets or history rewriting. Commit and push each meaningful completed step and relevant correction on the task branch.
- Run cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings, cargo test --locked and the Windows MSVC release build. Document platform and physical hardware checks separately.
- Changes to cryptography require authenticated state transitions and negative tests. Use established libraries; do not implement cryptographic algorithms. Trust confirmation must complete on both sides before chat.
- A release must run all checks for its exact commit before publication. Pin dependencies, actions and toolchain. Only the release job receives contents: write.
- Do not claim hardware, signing, security audit or publication results without evidence. Keep docs and CHANGELOG current.

- Keep the explicit Windows wgpu DX12 feature dependency and the native-start/backend/theme regressions. eframe's wgpu feature alone does not enable a Windows rendering backend.
- Windows session locking must use its own WTS notification worker; inactivity locking also runs in the actor. UI repaint timing must not control whether keys remain unlocked. Preserve generation-before-paint wiping and exact Enter/Shift+Enter handling.
- Probe release packaging on Windows before creating a version tag. Handle CRLF in version/lock metadata. Read a Windows lock sidecar for plaintext scans only after releasing the vault's mandatory file lock.
- After publication, download the actual release assets and verify tag commit, hashes, PE architecture/subsystem and ZIP identity. Build artifacts alone are not proof of published downloads.
