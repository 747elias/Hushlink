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

- Resume only application file bytes inside a fresh Noise session after renewed trust/consent. Bind checkpoints to identity, ID, name, size and full digest; validate prefix hashes; never persist cipher state/nonces. Preserve duplicate-receipt and abrupt-exit checkpoint tests.
- Notifications carry only generic text/counts. Bind notification state to lock generations and drive it from the native tray thread even without egui frames. Never include contact/file names or content previews.
- Drag/drop only queues local paths for the selected authenticated peer; require a separate explicit send action. Clear queued paths on lock, disconnect and recipient changes. Discovery results must match their uncancelled operation flag.

- Conversation actions require authenticated durable receipts, strict ownership/revision checks and persistent idempotence across fresh sessions. Keep unconfirmed changes visibly unconfirmed. Message/history deletion must retain content-free receipt digests against late retries; contact removal deletes them.
- Clipboard images require an explicit read in the selected trusted conversation and separate offer consent. Camera/clipboard workers use the runtime cancellation hub so native session lock cancels them without GUI frames; camera helpers belong to a kill-on-close Windows Job Object. QR comparison binds the entire fingerprint and never bypasses mutual confirmation.
- Windows Hello must remain real vault-key wrapping by the non-exportable Passport KSP with mandatory fresh user presence, failed silent access, and a verified enrollment roundtrip. No UI-consent/DPAPI substitute or weaker fallback. Authenticate the entire optional header extension and preserve password fallback.
- Backups contain authenticated ciphertext only. Never overwrite restore targets or prune files outside the exact owned backup namespace. Keep password-rotation, foreign-file retention and corruption tests. Old backups retain their old credentials/content.
- Release builds require authorized app and installer Authenticode signatures, trusted timestamps, expected certificate pin and exact-byte native installer tests before publication. Never bypass this gate to publish an unsigned substitute. The updater must verify the pinned publisher on its deny-write file handle before execution.
- Keep Media Foundation DLLs delay-loaded on MSVC and run the PE optional-camera check. Missing optional camera components must not prevent normal chat startup.

- Maintainer exception authorized on 2026-11-15: v0.4.0 may be published explicitly as an **unsigned prerelease**, using only commit `79723dcee7b5bd329e88a88e383bf4435e87e477` and the exact Windows assets verified by successful CI run `43707154440` and draft verification run `43708723288`. This single-release exception does not weaken updater signature verification or authorize unsigned future releases. Document the lack of signing and hardware tests visibly; verify actual public downloads after publication.
