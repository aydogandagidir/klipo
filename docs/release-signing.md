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

## 6. Authenticode signing (REQUIRED for v0.1.3+ commercial release)

Klipo v0.1.0–0.1.2 shipped without Authenticode signing — Windows SmartScreen showed "Publisher: Unknown" on first run. That was acceptable for the Apache-2.0 era, but **paying customers will not accept this**. Before listing on Gumroad, Klipo v0.1.3+ must be Authenticode-signed.

This is **independent of** the Tauri updater signing above; both are required and cover different threat models (SmartScreen reputation vs. update integrity).

### Option A — Azure Trusted Signing (recommended)

Cost: ~$10/month. No hardware token. No $300/yr sticker. Available since Mar 2024.

1. **Set up an Azure account** (if you don't have one). The first $200 of Azure credit is enough to cover the first ~18 months of signing.
2. **Create a Trusted Signing account + identity validation:**
   ```
   Azure Portal → Search "Trusted Signing" → Create account
     → Account name: bluedev-codesign
     → Region: West Europe (or nearest)
     → Pricing tier: Basic (~$9.99/mo)
   ```
3. **Identity validation:** As an individual sole proprietor (Aydoğan Dağıdır / bluedev), submit:
   - Government photo ID (passport or national ID).
   - Proof of address (utility bill, bank statement < 3 months old).
   - Validation typically completes in 1-3 business days.
4. **Create a certificate profile** in the Trusted Signing account → Certificate profiles → New → Type: "Public Trust Identity (Individual)".
5. **Set up a service principal** for CI access:
   ```
   az ad sp create-for-rbac --name "klipo-codesign-ci" \
     --role "Code Signing Certificate Profile Signer" \
     --scopes /subscriptions/<sub-id>/resourceGroups/<rg>/providers/Microsoft.CodeSigning/codeSigningAccounts/bluedev-codesign
   ```
   Save the `appId`, `tenant`, and `password` — these become the secrets below.
6. **Add GitHub repo secrets** (Settings → Secrets and variables → Actions):
   | Secret name | Source |
   |---|---|
   | `AZURE_TENANT_ID` | service principal `tenant` |
   | `AZURE_CLIENT_ID` | service principal `appId` |
   | `AZURE_CLIENT_SECRET` | service principal `password` |
   | `AZURE_CODESIGN_ENDPOINT` | e.g. `https://weu.codesigning.azure.net/` |
   | `AZURE_CODESIGN_ACCOUNT` | `bluedev-codesign` |
   | `AZURE_CODESIGN_PROFILE` | the profile name created in step 4 |
7. **Configure Tauri to call AzureSignTool** by adding `bundle.windows.signCommand` to `tauri.conf.json`:
   ```json
   "bundle": {
     "windows": {
       "signCommand": "AzureSignTool sign -kvu %AZURE_CODESIGN_ENDPOINT% -kvi %AZURE_CLIENT_ID% -kvt %AZURE_TENANT_ID% -kvs %AZURE_CLIENT_SECRET% -kvc %AZURE_CODESIGN_PROFILE% -tr http://timestamp.digicert.com -td sha256 %1"
     }
   }
   ```
8. **Install AzureSignTool in CI** by adding a step before `tauri-action`:
   ```yaml
   - name: Install AzureSignTool
     run: dotnet tool install --global AzureSignTool
   ```
9. **Verify** after the build:
   ```yaml
   - name: Verify Authenticode signature
     shell: pwsh
     run: |
       $exe = "src-tauri/target/release/bundle/nsis/Klipo_${{ env.VERSION }}_x64-setup.exe"
       signtool verify /pa /v $exe
   ```

Reference: https://learn.microsoft.com/azure/trusted-signing/

### Option B — EV Authenticode certificate (Sectigo / DigiCert / SSL.com)

Cost: ~$200–400/year. Requires hardware token (HSM) shipped to your address, OR cloud HSM (DigiCert KeyLocker, ~$25/mo).

Pros over Option A: instant SmartScreen reputation (EV-signed binaries skip the reputation-building phase). Best if you expect heavy first-week download volume.

Cons: more friction. The cert lives on a hardware token that must be physically present on the signing machine, OR in a cloud HSM that has its own integration overhead.

1. Buy an EV cert (Sectigo, DigiCert, SSL.com — pick one).
2. For hardware-token flow: the signing must run on a self-hosted Windows runner with the token attached. GitHub-hosted `windows-latest` won't work without KeyLocker.
3. For DigiCert KeyLocker: install KeyLocker tools on the runner, configure `bundle.windows.signCommand` to use `smctl sign` instead of `AzureSignTool`.
4. Add the appropriate secrets (`SM_HOST`, `SM_API_KEY`, `SM_CLIENT_CERT_FILE`, `SM_CLIENT_CERT_PASSWORD`).
5. Verification step is the same `signtool verify /pa /v ...` as in Option A.

### Verification (both options)

After tagging a release, run on a clean Windows 11 VM (no Klipo build tools):

```powershell
signtool verify /pa /v Klipo_0.1.3_x64-setup.exe
```

Expected output:
```
Successfully verified: Klipo_0.1.3_x64-setup.exe
```

Then double-click the installer and confirm SmartScreen no longer shows "Unknown publisher". The publisher should read "bluedev" or "Aydoğan Dağıdır" depending on what the cert profile is registered to.
