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
- File offers, consent, chunks, cancellation and storage receipts stay inside the authenticated session. Never release data before mutual trust and recipient consent; acknowledge only durable encrypted storage. Keep cancellation-race and lock-generation tests.
- All private data, including attachments and names, stays in the single encrypted vault. Preserve ciphertext copies on uncertain locator commits. Never automatically open attachments or create plaintext exports/backups.
- Unit tests must not change the real Windows Run registry. Test native installation/autostart on disposable Windows CI accounts; preserve user vaults on update and uninstall.
- Keep update hosts fixed, response/download limits, revalidation before launch and explicit install/restart. Respect automatic-update opt-out while locked. Checksums do not constitute a publisher signature.
- Native installer smoke must pass before tagging; release only the exact installer bytes that passed it. Preserve the bounded parent-process handoff before Inno AppMutex checks.
