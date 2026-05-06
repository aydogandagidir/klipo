# Klipo Cryptographic Envelope Spec — v0.1 Draft

**Status:** Draft. Locks before v0.3 (sync) implementation.
**Audience:** Engineers implementing key derivation and envelope encryption. Cryptography reviewers.
**Companion docs:** [`sync-protocol.md`](./sync-protocol.md), [`security.md`](./security.md).

---

## 1. Goals

- **Zero-knowledge sync.** Server never sees plaintext nor any key material.
- **Forward secrecy on transport.** TLS 1.3 + ephemeral X25519 during pairing.
- **Defense in depth.** Even if one device is compromised, vault data on other devices is rotatable.
- **Modern algorithms only.** Authenticated encryption (AEAD), no MAC-then-encrypt, no ECB.
- **Single library.** libsodium via the Rust `sodiumoxide` crate (or `dryoc` as fallback). **No OpenSSL.**
- **Side-channel awareness.** Constant-time primitives where libsodium provides them; no naive comparison of secrets.

## 2. Library Choice

| Option | Verdict |
|---|---|
| **libsodium (via `sodiumoxide`)** | **Chosen.** Auditing-friendly, constant-time, opinionated APIs (no algorithm soup). Widely deployed (Signal, WireGuard ecosystem). |
| `ring` | Strong but lower-level; we'd rebuild more glue. |
| `rustcrypto` family | Pure-Rust, but each primitive is a separate crate; less coherent API. Future fallback if `sodiumoxide` proves stale. |
| `openssl` / `aws-lc-rs` | Too much surface area; FIPS not a goal for a personal-data product. |
| Web Crypto in WebView | Cannot be used for at-rest encryption (key would be in JS heap). |

`sodiumoxide` is in maintenance mode upstream. **Backup plan:** if it goes stale, switch to `dryoc` (pure-Rust libsodium-compatible) — same primitives, same wire format, drop-in for our use cases.

## 3. Algorithms (Locked)

| Purpose | Algorithm | libsodium API |
|---|---|---|
| Password KDF | **Argon2id** | `crypto_pwhash` |
| Key encapsulation | **X25519** ECDH | `crypto_kx_*`, `crypto_box_*` |
| Symmetric AEAD | **XChaCha20-Poly1305** (24-byte nonce) | `crypto_aead_xchacha20poly1305_ietf_*` |
| Digital signature | **Ed25519** | `crypto_sign_*` |
| Hash | **BLAKE2b-256** | `crypto_generichash` |
| HMAC alternative | **BLAKE2b keyed** | `crypto_generichash` w/ key |
| KDF (HKDF-like) | **BLAKE2b-based KDF** | `crypto_kdf` |
| Random | OS CSPRNG via libsodium | `randombytes_buf` |

**Forbidden:** AES-CBC, RSA, SHA-1, MD5, custom XOR, "encrypted then base64'd as auth", CTR without MAC.

---

## 4. Key Hierarchy

```
                       ┌──────────────────────────┐
   user password ─────►│  Argon2id (memory-hard)  │
                       │  salt = vault.salt       │
                       └────────────┬─────────────┘
                                    │ 32 bytes
                                    ▼
                        ┌──────────────────────────┐
                        │   Master Vault Key (MVK) │  Never leaves device unsealed
                        │   (32 bytes, secret)     │  Held in mlock'd memory only
                        └────────────┬─────────────┘
                                     │
              ┌──────────────────────┼─────────────────────────────────┐
              │                      │                                 │
              ▼                      ▼                                 ▼
   ┌──────────────────┐   ┌──────────────────────┐         ┌─────────────────────┐
   │  Records Key     │   │  Settings Key        │         │  Index Key (search) │
   │  KDF(MVK, "rec") │   │  KDF(MVK, "set")     │         │  KDF(MVK, "idx")    │
   └────────┬─────────┘   └──────────────────────┘         └─────────────────────┘
            │
            ▼ per-clip
   ┌────────────────────┐
   │ Random 24B nonce   │
   │ XChaCha20-Poly1305 │
   └────────────────────┘
```

**Per-device** (separate from vault hierarchy):

```
   ┌──────────────────────────┐
   │  Device Identity Keypair │
   │  Ed25519 (sign/verify)   │
   └────────────┬─────────────┘
                │ stored sealed via OS:
                │   Windows: DPAPI (CurrentUser scope)
                │   macOS:   Keychain (kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
                ▼

   ┌──────────────────────────┐
   │  Pairing Ephemeral X25519│  Generated per pairing, discarded after.
   └──────────────────────────┘
```

## 5. Argon2id Parameters

Tuned for ~250ms on 2026-era mid-range laptop (Ryzen 7 / M2 / Snapdragon X). Calibration script writes results to keychain so future unlocks reuse same params.

**Defaults (v0.1):**

```
opslimit  = 3              # iterations
memlimit  = 256 * 1024 * 1024  # 256 MiB
parallelism = 1            # libsodium fixes this
salt = randombytes_buf(16) # per vault, stored alongside vault metadata
```

**Calibration:** On first vault creation, run a 4-point ladder (1/64MB, 2/128MB, 3/256MB, 4/512MB), pick the highest tier that completes in <300ms. Save params.

**Rationale:** OWASP 2023 minimum recommendations are `opslimit=2, memlimit=64MB`. We're 4× stronger. Cost on low-end devices: 1-2s unlock — acceptable for daily use.

## 6. Vault Master Key Wrapping

The Master Vault Key (MVK) lives in two states:

1. **Sealed (at rest):** Encrypted under user password via Argon2id.
2. **Unsealed (in memory):** Plain bytes in mlock'd buffer; zeroized on unlock-app close, lock timeout, or explicit lock.

### 6.1 Sealing

```
seed = randombytes_buf(32)          // optional; not used in v0.3
key  = Argon2id(password, salt)     // 32 bytes
nonce = randombytes_buf(24)
sealed_mvk = XChaCha20Poly1305_encrypt(MVK, key, nonce, aad="klipo-vault-mvk-v1")

stored_blob = {
    "version": 1,
    "salt": <16B>,
    "argon2_params": { "ops": 3, "mem": 268435456 },
    "nonce": <24B>,
    "ciphertext": <48B>     // 32B MVK + 16B Poly1305 tag
}
```

### 6.2 Unsealing

Reverse. Verify AEAD tag (libsodium does this) → if fails, password wrong → no further attempts beyond local rate-limit (3 fails → 30s lockout, exponential).

### 6.3 Multi-Device Wrapping

When a new device pairs, MVK is re-wrapped under that device's pairing key (see [`sync-protocol.md`](./sync-protocol.md) §7). Server stores no wrapped MVK.

### 6.4 Rotation

```
new_mvk = randombytes_buf(32)

For each clip in vault:
    plaintext = decrypt_with_old_records_key(clip.ciphertext)
    new_records_key = KDF(new_mvk, "rec")
    new_ciphertext = encrypt_with_new_records_key(plaintext)
    submit_replacement_record(clip.id, new_ciphertext, hlc=now)

For each remaining device:
    rewrap MVK under device's pubkey (see pairing flow in sync-protocol.md §8.2)
    push rotation envelope record
```

Old records remain readable until each device confirms migration via heartbeat ("at hlc X, I can read v2"). After 14 days, old MVK can be destroyed locally; in true compromise scenario, immediate destruction is preferred (accepts that some records may be unreadable on un-migrated devices).

---

## 7. Per-Clip AEAD Envelope

```
plaintext  = serialize_cbor(clip_record)         // see sync-protocol.md §5
records_key = KDF(MVK, "klipo-records-v1")        // 32 bytes
nonce       = randombytes_buf(24)                 // 192-bit, no collision risk
aad         = vault_id || record_id || hlc        // bound to identity
ciphertext  = XChaCha20Poly1305_encrypt(plaintext, records_key, nonce, aad)

wire_blob = nonce || ciphertext                   // 24 + N + 16 (tag) bytes
```

**Why XChaCha20-Poly1305 not ChaCha20-Poly1305 (12B nonce)?**
- 24-byte random nonces have negligible collision probability over 2^48 messages — safe to forget about counters.
- 12-byte nonces require careful counter management; mistakes catastrophic (key leak via nonce reuse).

**Why include `record_id` and `hlc` in AAD?**
- Prevents server from swapping ciphertexts between records (would fail AEAD tag verification).
- Prevents replay of an old record under a new id.

## 8. Device Identity & Tokens

### 8.1 Device Keypair

```
(pk_dev, sk_dev) = crypto_sign_keypair()    // Ed25519
```

Stored sealed in OS keychain. Never written to disk plaintext.

### 8.2 Device Token (Auth)

The server issues short-lived bearer tokens, but each token's payload is signed by the **device** to prove possession. This sidesteps PAKE complexity in v0.3 while still preventing token theft from being trivially exploitable.

```
token_payload = {
    "vault_id": "...",
    "device_id": "...",
    "exp": <unix_seconds>,       // 1h from now
    "nonce": <16B random>
}
device_signature = crypto_sign_detached(token_payload, sk_dev)

bearer_token = base64(token_payload) + "." + base64(device_signature)
```

Server verifies signature against `pk_dev` registered at pairing time. Stolen token (without sk_dev) cannot be replayed past expiry.

**v1.0 upgrade path:** Replace bearer with OPAQUE-style PAKE handshake (mutual zero-knowledge auth). Spec'd separately when we get there.

## 9. Pairing Key Derivation

(Cross-reference: [`sync-protocol.md`](./sync-protocol.md) §7.2)

```
psk = randombytes_buf(32)         // shown to user via QR

(eph_pk_A, eph_sk_A) = crypto_box_keypair()
(eph_pk_B, eph_sk_B) = crypto_box_keypair()

shared_A = crypto_scalarmult(eph_sk_A, eph_pk_B)
shared_B = crypto_scalarmult(eph_sk_B, eph_pk_A)
// shared_A == shared_B (32 bytes)

pairing_key = BLAKE2b(shared || psk, len=32, personalization="klipo-pair-v1")
```

This `pairing_key` then encrypts the MVK over the relay channel.

## 10. Sensitive Content In-Memory Lifecycle

For clips marked `sensitive=true`:

1. Plaintext lives in a `Zeroizing<Vec<u8>>` buffer (Rust crate `zeroize`).
2. Buffer is `mlock`'d (`mlock` on Unix, `VirtualLock` on Windows) so OS swap won't hit disk.
3. Tokio task scheduled at clip insert+30s zeroizes & frees the buffer.
4. UI requesting display fetches via a re-decrypt path (slower but bounded).

**Crucially:** sensitive content is **still persisted to disk encrypted** (otherwise we'd lose history). The 30s window only purges the unencrypted in-RAM cache.

## 11. Test Vectors (KAT — Known Answer Tests)

Stored in `bench/tests/kat.json`. Each test vector specifies inputs and expected ciphertext bytes, exercised in CI.

### 11.1 Argon2id KAT

```json
{
  "name": "argon2id-vault-key",
  "password": "correct horse battery staple",
  "salt": "00112233445566778899aabbccddeeff",
  "ops": 3,
  "mem_kib": 65536,
  "expected_key_hex": "<filled by reference impl>"
}
```

(Generated by libsodium reference; we re-run on every build to ensure no library drift.)

### 11.2 XChaCha20-Poly1305 KAT

Three vectors covering: empty plaintext, 1B plaintext, 1KiB random plaintext. Generated from libsodium reference; failure means a dependency upgrade silently broke compatibility.

### 11.3 X25519 KAT

RFC 7748 §6.1 vectors. Sanity check our `crypto_scalarmult` invocation matches RFC.

## 12. Crypto API Surface (Rust Trait)

```rust
// src-tauri/src/crypto/mod.rs (Phase D)

pub trait VaultCrypto: Send + Sync {
    /// Derive MVK from password + salt + Argon2id params.
    fn derive_master_key(
        password: &str,
        salt: &[u8; 16],
        params: Argon2Params,
    ) -> Result<MasterKey, CryptoError>;

    /// Seal MVK for at-rest storage on this device.
    fn seal_master_key(
        mvk: &MasterKey,
        password: &str,
        salt: &[u8; 16],
        params: Argon2Params,
    ) -> Result<SealedMVK, CryptoError>;

    /// Unseal previously sealed MVK.
    fn unseal_master_key(
        sealed: &SealedMVK,
        password: &str,
    ) -> Result<MasterKey, CryptoError>;

    /// Encrypt a record (clip/pin/tombstone) for sync.
    fn encrypt_record(
        plaintext: &[u8],
        records_key: &RecordsKey,
        aad: &Aad,
    ) -> Result<EncryptedRecord, CryptoError>;

    /// Decrypt a record received from server.
    fn decrypt_record(
        encrypted: &EncryptedRecord,
        records_key: &RecordsKey,
        aad: &Aad,
    ) -> Result<Vec<u8>, CryptoError>;

    /// Generate Ed25519 device keypair.
    fn generate_device_keypair() -> (DevicePublicKey, DeviceSecretKey);

    /// Sign a token payload with device key.
    fn sign_token(
        payload: &[u8],
        sk: &DeviceSecretKey,
    ) -> [u8; 64];
}

#[derive(zeroize::ZeroizeOnDrop)]
pub struct MasterKey([u8; 32]);

#[derive(zeroize::ZeroizeOnDrop)]
pub struct RecordsKey([u8; 32]);

#[derive(Debug)]
pub enum CryptoError {
    InvalidPassword,
    AeadTagMismatch,
    InvalidNonce,
    KeyDerivation(String),
    LibsodiumInitFailed,
    InvalidParams,
}
```

All secret types implement `ZeroizeOnDrop`. Compiler-enforced via Rust's ownership.

## 13. Quantum Readiness Note

For 2026, classical algorithms suffice. **Roadmap:**

- **v1.0 (2027 target):** Hybrid pairing key derivation:
  ```
  pairing_key = HKDF(X25519_shared || ML-KEM-768_shared || psk)
  ```
  Doubles pairing message size (~1.5KB extra), trivial in QR-flow context (use code, not raw QR).
- **v2.0:** Hybrid signatures (Ed25519 + ML-DSA-65) for device tokens.

We do NOT plan to encrypt records under PQ schemes in v1.x — symmetric XChaCha20 is post-quantum-secure for an additional 100+ years given Grover's halving.

## 14. Side Channels

| Channel | Mitigation |
|---|---|
| Timing on AEAD failure | libsodium constant-time; we never branch on tag bytes |
| Timing on password compare | We don't compare passwords; we use Argon2id KDF + AEAD verify |
| Timing on cursor/db lookups | Acceptable — server-side correlation is information we don't ban |
| Memory disclosure (process dump) | mlock + zeroize for secrets; we accept dump-during-unlock as out of scope |
| Cold-boot RAM scraping | Defer to OS disk encryption + screen-lock |
| Compiler reordering of zeroize | `zeroize` crate uses `volatile` writes + `compiler_fence` |

## 15. Failure Modes

- **Argon2id calibration unstable across CPUs.** Acceptable — we calibrate per device, not centrally.
- **OS keychain unavailable** (rare, e.g., corrupted Windows DPAPI store). Detect, surface error, fall back to in-memory only with strong warning ("vault will not survive logout").
- **`mlock` rejected by OS** (rlimits). Fall back to soft-zero (still zeroize on drop, but accept that swap may have written it). Log warning.
- **libsodium init fails.** Refuse to start; clipboard manager without crypto is a no-go for v0.3+.
- **Rotation interrupted mid-flight.** Idempotent: each clip's re-encryption is a single record submission; partial migration is fine — old MVK stays alive 14d.

## 16. Reviews & Sign-Off

This spec is a draft. **Required before v0.3 ships:**

- [ ] One independent review by an engineer with cryptographic background (peer in security community).
- [ ] Threat model in [`security.md`](./security.md) cross-referenced for completeness.
- [ ] KATs running in CI green.
- [ ] Spec frozen and given a version (`crypto-v1`); future changes follow semantic versioning with explicit migration notes.

## 17. References

- D. J. Bernstein et al., *XSalsa20*, 2008 (XChaCha20 derivation).
- A. Biryukov et al., *Argon2: the memory-hard function for password hashing*, 2016.
- IETF RFC 7748 (X25519/X448), RFC 8032 (Ed25519), RFC 8439 (ChaCha20-Poly1305), RFC 9106 (Argon2).
- Frank Denis, *libsodium documentation*, https://doc.libsodium.org/.
- Trevor Perrin, *The Noise Protocol Framework*, rev 34, 2018 — for pairing inspiration.
