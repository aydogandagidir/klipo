# Release Signing Setup

Klipo's auto-update path verifies every manifest with an Ed25519 signature. The plugin will refuse any update that doesn't match the embedded `pubkey` in `tauri.conf.json` — that's the whole reason auto-update can be safe in the first place. This doc walks through generating the keypair, wiring it into the build, and confirming end-to-end that update verification works.

> **Status of the repo:** A placeholder `pubkey` is already committed at `src-tauri/tauri.conf.json` so dev builds compile and the "Check for updates" button shows a friendly "Updates not configured for this build" message instead of crashing. Replace the placeholder with the real public key produced below before tagging a release.

---

## 1. Generate the keypair (one time, do this on a trusted machine)

The Tauri CLI ships a signer subcommand. From the repo root:

```powershell
pnpm tauri signer generate -w klipo-updater.key
```

You will be prompted for a passphrase. **Use a strong one.** The passphrase encrypts the `klipo-updater.key` file at rest; without it the private key is unusable.

Outputs:

- `klipo-updater.key` — the private key (encrypted with your passphrase). **Never commit.** Keep it in a password manager or a sealed envelope.
- `klipo-updater.key.pub` — the public key, safe to commit / share.

---

## 2. Wire the public key into the build

Open `src-tauri/tauri.conf.json` and replace the placeholder:

```json
"plugins": {
  "updater": {
    "active": true,
    "endpoints": [
      "https://github.com/aydogandagidir/klipo/releases/latest/download/latest.json"
    ],
    "dialog": false,
    "pubkey": "<paste the contents of klipo-updater.key.pub here, single line>"
  }
}
```

If the release host changes (e.g. forking the repo), update the endpoint URL above to match the new repository.

Commit `tauri.conf.json` (the public key is meant to be public — it's how every running Klipo verifies that an update came from you).

---

## 3. Wire the private key into CI

The CI job that runs `pnpm tauri build` needs both the private key file and its passphrase to sign each release. Set them as **GitHub repository secrets** (Settings → Secrets and variables → Actions):

| Secret name | Contents |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Full contents of `klipo-updater.key` (the encrypted file) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The passphrase you chose in step 1 |

The existing `.github/workflows/release-windows.yml` already passes these through to `tauri build` via the env block — no further workflow changes are needed.

> The older Tauri 1.x env names (`TAURI_PRIVATE_KEY`, `TAURI_KEY_PASSWORD`) are also accepted; the workflow uses the v2 names since this repo is on Tauri 2.

---

## 4. Verify end-to-end (one full release cycle)

The only way to be sure signing is wired correctly is to ship one full release and test the update path on a second machine.

1. **Tag and push a release.**
   ```bash
   git tag v0.1.0
   git push --tags
   ```
   GitHub Actions runs `release-windows.yml`, produces a draft Release with the MSI, NSIS, and `latest.json`, and signs `latest.json` with `TAURI_SIGNING_PRIVATE_KEY`.

2. **Inspect `latest.json`.** Download it from the draft Release and confirm:
   - It contains a top-level `signature` field (long base64 string).
   - The version, notes, and download URL inside match what you pushed.

3. **Install the MSI on machine A.** Run Klipo. It says "v0.1.0".

4. **Tag a v0.1.1 hotfix and push it.** A second draft Release shows up with a new `latest.json`.

5. **Click "Check for updates" in machine A's Klipo Settings → General.**
   - Expected: "Update available: 0.1.1" with the release notes.
   - If you see "Updates not configured for this build": the placeholder pubkey is still in `tauri.conf.json`. Re-do step 2.
   - If you see a signature-verification error: the CI built with a different private key than the public key embedded in the binary. Re-check that both `TAURI_SIGNING_PRIVATE_KEY` and `tauri.conf.json` came from the same `pnpm tauri signer generate` run.

6. **Click "Download and install".** The new installer runs and replaces the running binary. Klipo restarts on its own with the new version.

If all six steps pass once, signing is correct and you don't need to touch this again until you rotate the key.

---

## 5. Key rotation

Plan to rotate the signing key roughly once a year, and immediately if it leaks.

Procedure:

1. Generate a new keypair (`klipo-updater-2.key`).
2. Update `tauri.conf.json` with the new public key.
3. Update `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` GitHub secrets.
4. Tag a new release. Existing users running the OLD binary trust ONLY the OLD key — they cannot install updates signed with the new key. So:
5. Ship a "transition" build first: keep BOTH the old AND new pubkey in `tauri.conf.json` (Tauri accepts an array — `pubkey: ["old-key", "new-key"]`). Sign the transition release with the OLD key so existing users can install it.
6. Once enough users are on the transition build, retire the old key by removing it from `tauri.conf.json` and signing future releases only with the new key.

Don't skip step 5 — without it, anyone on the old build is stranded.

---

## 6. What's NOT signed (yet)

The Klipo MSI itself is not Authenticode-signed in v0.1. Windows SmartScreen will show a "Publisher: Unknown" warning the first time a user runs the installer. To fix this:

1. Acquire an EV (Extended Validation) code-signing certificate (~$300/year).
2. Add a `signtool sign` step to `release-windows.yml` between `tauri build` and the upload to GitHub Release.
3. Cert + private key live as `WINDOWS_CERT_BASE64` + `WINDOWS_CERT_PASSWORD` GitHub secrets.

EV cert is independent of the Tauri updater signing above — they cover different threat models (SmartScreen reputation vs. update integrity). Ship the updater signing first; EV cert can land in v0.1.x if budget allows.
