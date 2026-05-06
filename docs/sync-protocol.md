# Klipo Sync Protocol Specification — v0.1 Draft

**Status:** Draft. Locks before v0.3 (Phase D) implementation begins.
**Audience:** Engineers building the sync client/server. Security reviewers.
**Out of scope:** Crypto envelope details (see [`crypto.md`](./crypto.md)) and storage layout (see [`storage.md`](./storage.md)).

---

## 1. Goals & Non-Goals

### Goals

- **Eventually consistent multi-device sync** of clipboard items, pinned states, and tombstones.
- **Zero-knowledge server.** Server stores ciphertext + opaque metadata only; cannot read content, sender app, or per-clip timestamps in plaintext.
- **Offline-tolerant.** Devices may be offline for days, then catch up without conflict storms.
- **Deterministic conflict resolution.** Two devices that diverged for an hour produce the same final state once they reconcile, regardless of merge order.
- **Bandwidth-cheap.** A device that's been offline 24h syncs in <2s on a 10 Mbps link with ~500 new clips.
- **Deletable.** Soft delete propagates within seconds; hard delete after retention.
- **Revocable.** Lost device can be ejected; future sync from that device is rejected.

### Non-Goals (v0.3)

- **Real-time collaboration** with sub-second push fan-out. Polling + a lightweight push notification is sufficient for v0.3.
- **Operational Transform / full CRDT semantics for rich text.** Clips are immutable blobs; we don't merge content.
- **Server-side search.** All search runs locally on each device against decrypted state.
- **Cross-account sharing.** Per-clip ACL is v1.0+ territory.

---

## 2. Conceptual Model

A **vault** belongs to one user. A vault has many **devices** (Klipo installs) paired to it. Devices store and sync **records** through a server.

A **record** is one of:
- **Clip** — clipboard item (text/image/file).
- **Pin update** — change of pinned flag on an existing clip.
- **Tombstone** — soft-delete marker.
- **Settings delta** — per-vault setting changes (e.g., excluded apps list).

Records are **immutable once written**. Updates to mutable state (pinned flag) are modeled as new records that supersede prior ones via LWW.

```
Vault
 ├─ Device A (Windows)  ─┐
 ├─ Device B (Mac)       ├─► Server (zero-knowledge relay)
 ├─ Device C (iOS r/o)   ┘
 └─ Settings (synced)
```

---

## 3. CRDT Choice: LWW-Element-Set + Tombstone

Considered:

| Option | Pros | Cons | Decision |
|---|---|---|---|
| Yjs / Automerge | Battle-tested, rich-text aware | Overkill for immutable blobs; large binary delta protocol | **Rejected** |
| OR-Set | Add/remove without ambiguity | Still complex, same outcome for our access pattern | **Rejected** |
| **LWW-Element-Set + tombstone** | Trivial to reason about; idempotent; cheap | LWW loses info on simultaneous mutation (acceptable for clipboard) | **Chosen** |

Rationale: Clipboard items are **append-only**. The only mutable bits are `pinned` (rare) and `deleted_at` (one-shot transition). True merge conflicts are vanishingly rare; a "winner takes all by HLC timestamp" rule is correct enough.

### LWW Rules

- Each record carries an HLC timestamp (see §4) and the writing device's id.
- For a given `clip_id`, the record with the **highest HLC** wins. Ties broken by **lexicographically larger device id**.
- Tombstones are sticky: once a clip is tombstoned, no later non-tombstone record for the same id is accepted (server enforces this; client rejects stale incoming records).

---

## 4. Hybrid Logical Clock (HLC)

**Why not Lamport?** Lamport clock loses physical time intuition. Users expect "things copied today" to sort above "things copied yesterday" even after sync. HLC keeps physical time within a tunable bound and is monotonic.

**Why not pure wall clock?** Devices have skewed clocks. A 2-minute-fast laptop would always "win" merges.

### HLC Format

64-bit packed:
- **48 bits:** physical time, milliseconds since Unix epoch (covers ~8900 years).
- **16 bits:** logical counter, increments when physical time hasn't advanced.

```
hlc = (physical_ms << 16) | logical_counter
```

### Update Rules (per device, ref. Kulkarni et al. 2014)

```
On local event:
    pt_now = wall_clock_ms()
    if pt_now > pt_last:
        pt_last = pt_now
        l_last  = 0
    else:
        l_last += 1
    return (pt_last << 16) | l_last

On receiving message with hlc m:
    pt_now = wall_clock_ms()
    pt_m   = m >> 16
    l_m    = m & 0xFFFF
    pt_new = max(pt_last, pt_m, pt_now)
    if pt_new == pt_last == pt_m:
        l_last = max(l_last, l_m) + 1
    elif pt_new == pt_last:
        l_last += 1
    elif pt_new == pt_m:
        l_last = l_m + 1
    else:
        l_last = 0
    pt_last = pt_new
    return (pt_last << 16) | l_last
```

**Drift bound:** If `wall_clock_ms()` skew exceeds 2 minutes, the device emits a warning and refuses to sync until NTP-corrected. Hard limit prevents an attacker from ejecting recent clips by claiming a future timestamp.

---

## 5. Data Frames

All sync messages are **encrypted blobs** of the following plaintext shape (encrypted via crypto envelope, see [`crypto.md`](./crypto.md)):

### 5.1 Clip Record (plaintext, before encryption)

```json
{
  "type": "clip",
  "id": "01J9X8VK3T4M5N6P7Q8R9STUVW",
  "hlc": "00018F7BDA34C001",
  "device_id": "0e4a...d9",
  "kind": "text",
  "content_hash": "sha256:9b74c9897bac770...",
  "size_bytes": 1024,
  "source_app": "Code.exe",
  "source_url": null,
  "created_at_ms": 1746355200000,
  "sensitive": false,
  "category": null,
  "blob_ref": null,
  "text_content": "...full text or omitted if blob_ref present..."
}
```

For binary clips (image/file), `text_content` is null and `blob_ref` is a server URL pointing to a separately-uploaded encrypted blob:

```json
{
  ...,
  "kind": "image",
  "blob_ref": "blob://01J9X8VK3T4M5N6P7Q8R9STUVW.bin",
  "content_hash": "sha256:..."
}
```

### 5.2 Pin Update Record

```json
{
  "type": "pin",
  "id": "01J9...",
  "clip_id": "01J9X8VK3T4M5N6P7Q8R9STUVW",
  "pinned": true,
  "hlc": "00018F7BDB001234",
  "device_id": "0e4a...d9"
}
```

### 5.3 Tombstone Record

```json
{
  "type": "tombstone",
  "id": "01J9...",
  "clip_id": "01J9X8VK3T4M5N6P7Q8R9STUVW",
  "hlc": "...",
  "device_id": "0e4a...d9",
  "reason": "user_delete"
}
```

`reason` is opaque to the server but stored locally for audit. Allowed values: `user_delete`, `retention_expiry`, `policy_violation` (e.g., sensitive content auto-purge).

### 5.4 Settings Delta Record

```json
{
  "type": "settings",
  "id": "01J9...",
  "key": "excluded_apps",
  "value": "[\"vault-app.exe\",\"secret-keeper.exe\"]",
  "hlc": "...",
  "device_id": "..."
}
```

Last-write-wins per `key`. Settings sync is opt-in and can be disabled per device (e.g., "this Windows machine has different excluded apps than my Mac").

---

## 6. Server Wire Protocol

**Transport:** HTTPS with TLS 1.3 only. mTLS optional for self-host.
**Encoding:** All bodies are CBOR (RFC 8949). Smaller and faster than JSON. JSON variant offered only for debug builds.
**Auth:** Per-device signed token (Ed25519 over `device_id || vault_id || expiry`); short-lived (1h), refreshed by long-lived refresh token in OS keychain.

### 6.1 Endpoints

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/v1/vaults/{vid}/records` | Push records (batch up to 256). |
| `GET`  | `/v1/vaults/{vid}/records?since={cursor}&limit={n}` | Pull records since cursor. |
| `POST` | `/v1/vaults/{vid}/blobs` | Pre-signed URL request for binary blob upload. |
| `GET`  | `/v1/vaults/{vid}/blobs/{bid}` | Download (pre-signed URL). |
| `POST` | `/v1/vaults/{vid}/devices` | Pair new device (PSK-authenticated). |
| `DELETE` | `/v1/vaults/{vid}/devices/{did}` | Revoke device (any pairing-master can do this). |
| `POST` | `/v1/vaults/{vid}/devices/{did}/heartbeat` | Liveness ping. |

### 6.2 Cursor Semantics

The `since` cursor is an **opaque server-generated** token, NOT an HLC. Why: revealing HLC ranges leaks user activity windows to a compromised server. Cursor encodes (server_seq, server_seq_continuation) and is monotonic per vault.

### 6.3 Batch Push Semantics

```
POST /v1/vaults/{vid}/records
Content-Type: application/cbor
Authorization: Bearer <device_token>

[
  { "id": "01J9...", "ciphertext": <bytes>, "kind": "clip" },
  { "id": "01J9...", "ciphertext": <bytes>, "kind": "tombstone" },
  ...
]
```

- Server checks token, validates each record id is unique (rejects duplicates with 409).
- Server stores ciphertext + minimal metadata: `(record_id, vault_id, kind, server_seq, received_at, ciphertext_size)`. **Crucially, no HLC in plaintext metadata.**
- Server returns `{ accepted: [...], rejected: [{id, reason}, ...] }`.
- Clients persist server's seq cursor on success.

### 6.4 Pull

```
GET /v1/vaults/{vid}/records?since=<cursor>&limit=256
→ 200 OK
[
  { "id": "...", "ciphertext": <bytes>, "kind": "clip", "received_at_ms": ... },
  ...
  { "next_cursor": "..." }
]
```

Client decrypts each record, runs LWW resolution against local DB, and updates state. If decryption fails → log + skip + report to telemetry (opt-in) without exposing key material.

---

## 7. Pairing Flow

**Goal:** Add a new device to an existing vault without sending the master key over an insecure channel.

### 7.1 Bootstrap (First Device)

1. User runs onboarding, sets a vault password.
2. Device generates Ed25519 device keypair (kept in OS keychain).
3. Master vault key = `Argon2id(password, vault_salt)` — see [`crypto.md`](./crypto.md).
4. Device registers with server: `POST /v1/vaults` (creates vault if password proves possession via OPAQUE-style PAKE in v1.0; v0.3 uses bearer-token + email verification).

### 7.2 Pairing a Second Device

```
Device A (already paired)                 Device B (new)
─────────────────────────                 ──────────────
Show "Add device" QR                      
  contains: pairing_psk (32B random),     Scan QR
            vault_id,                     Decode psk + vault_id
            relay_token (15min TTL)
                                          Generate B's Ed25519 keypair
Listen on relay endpoint                  Connect to relay (HTTPS WebSocket)
                                          
                                          Send: ECDHE(X25519) pubkey_B
Send: ECDHE(X25519) pubkey_A
                                          
Both: shared = ECDH(priv, peer_pub)
      key    = HKDF(shared, psk, "klipo-pairing-v1")

Encrypt vault_master_key with `key`
Send to B                                 Decrypt vault_master_key
                                          Store in OS keychain
Pairing complete                          B registers with server using new token
```

**Security properties:**
- PSK in QR prevents MitM at the relay.
- Forward secrecy from ephemeral X25519.
- Relay token expires in 15min — narrow window for attack.
- Server sees ciphertext only; cannot extract vault key.

### 7.3 Out-of-Band Verification

After pairing, both devices display a **6-word verification phrase** (BIP-39-style) derived from `BLAKE2b(shared_key)`. User compares phrases on both screens. Mismatch → revoke and re-pair.

---

## 8. Revocation & Key Rotation

### 8.1 Revoke Device

Any device with **revocation privilege** (default: all devices have it; user can promote one as "primary") can call:

```
DELETE /v1/vaults/{vid}/devices/{did}
Authorization: Bearer <revoker_token>
Body: { "reason": "lost_phone" }
```

Server marks the device's tokens invalid. **Important:** This does NOT rotate the vault key, so any data the revoked device already pulled is forever in attacker's hands. Mitigation: rotate.

### 8.2 Vault Key Rotation

Triggered by:
- Manual user request.
- Automatic on revocation (default for primary device revoking).

Rotation flow:
1. Primary device generates new master key.
2. For each remaining device, re-wrap master key with that device's pubkey, push as a new "rotation envelope" record.
3. Old ciphertext stays accessible until each device has both keys; new records use new key.
4. Server is told: "as of HLC X, prefer key v2."

For full re-encryption of historical clips, see [`crypto.md`](./crypto.md) §6.

---

## 9. Worked Example: 24-Hour Catch-Up

Device A is the primary, Device B has been offline 24h with the laptop closed.

```
Time         Device A                       Device B
────         ──────────────────             ──────────────────
T-24h        Online, normal use             Online, normal use
T-23h..T-1h  Generates 412 clips,           Offline (lid closed)
             5 pins, 8 deletes (HLCs ascending)
                                            
T0           User opens lid                  Wakes up
T+1ms                                        Reads server cursor from local DB
T+50ms                                       GET /v1/vaults/X/records?since=cursor
T+200ms                                       ← 256 records, next_cursor
T+250ms                                       Decrypts & merges (LWW). Fast — no
                                              real conflicts because A's HLCs > B's
T+450ms                                       GET /v1/vaults/X/records?since=cursor
T+650ms                                       ← 169 records, end of stream
T+700ms                                       Pushes any local-only deltas (none here)
T+750ms                                       UI updated; user resumes work
```

Total wall-clock ≈ 750ms for 425 records on local LAN. Pessimistic 4G (~150ms RTT × 4 round trips + transfer): ~1.5s.

---

## 10. Failure Modes & Recovery

| Failure | Detection | Recovery |
|---|---|---|
| Decryption failure on pulled record | AEAD tag mismatch | Skip, log `(record_id, error)`, continue. After 10 failures in a row, alert user. |
| HLC clock skew >2min | Comparing device pt vs incoming pt | Refuse sync; prompt user to NTP-sync. |
| Server rejects push (409 conflict on id) | 409 status | Server has older copy of same id. Compare HLCs locally. If our copy is newer, force-push wins (this is rare; UUID collisions are vanishingly improbable). |
| Server unreachable | Network error or 5xx | Exponential backoff (1s, 2s, 4s, ..., max 5min). Hold local writes; flush when reachable. |
| Vault key mismatch | Decryption fails on ALL recent records | User likely changed password on another device. Trigger reauthentication flow. |
| Device token expired | 401 status | Refresh token from keychain; if refresh fails, force re-login. |
| Server tampered with cursor (rewinds a device) | Client sees record with `received_at_ms` older than its prior cursor's record | Log security event; do NOT trust the rewind; resume from last-known-good cursor. |

---

## 11. Privacy Properties Server Cannot Violate

The server, even if fully compromised, cannot determine:

- **Clip content.** AEAD-encrypted with vault key the server never has.
- **Per-clip wall time.** `created_at_ms` is in plaintext only inside the encrypted body. Server sees `received_at_ms` (which is server-tarafı).
- **Source app or URL.** Inside encrypted body.
- **Number of pinned items.** Pin records are indistinguishable from clip records by server-visible metadata except `kind`. Even `kind` could be elided in v1.0 (currently kept for server-side blob-vs-record routing).
- **Whether two devices belong to the same human.** Vaults are pseudonymous; only pairing relays carry device-pair info, and those messages are ephemeral.

Server **can** see (acceptable):
- Approximate **upload volume per vault per day** (rate-limit signal).
- **Active hours** (when pushes/pulls occur).
- **Total ciphertext size** in vault.
- **Device count per vault.**

Mitigation for traffic analysis: in v1.0, optional **dummy traffic** (1 record/min cover noise during active hours).

---

## 12. v0.1 Scope

**Phase A produces this spec.** Phase B (Windows v0.1) implements **none of this** — sync is v0.3 (Phase D).

What v0.1 must NOT do that would be hard to retrofit:
- Make `clip.id` choice now: **UUIDv7** (lexicographically sortable by time, 128-bit, low collision risk). Don't use auto-increment.
- Reserve `sync_version INTEGER` column in clips table from day one (already in PRD schema).
- Reserve HLC field as nullable in v0.1 schema; required in v0.3 migration.
- Settle on CBOR vs JSON for wire format now (decision: **CBOR**; JSON for debug only).

---

## 13. Open Questions for v0.3 Decision

- [ ] Self-host server: Cloudflare Workers + D1 + R2 (PRD assumption) vs Postgres + S3-compatible vs Litestream + SQLite. **Recommendation:** CF Workers + D1 + R2 for managed; provide Docker compose with Postgres + MinIO for self-host. Two implementations of the same protocol.
- [ ] Push notifications for "new clip from another device" — APNs/FCM (requires server holds device tokens; acceptable since no plaintext exposed) vs WebSocket-only (battery cost on mobile).
- [ ] Per-clip TTL hints from sender ("don't sync this past tomorrow") — advanced, defer to v1.0.
- [ ] PAKE (OPAQUE) for password-derived auth — strongly preferred over bearer + email; prototype in v0.3, stabilize in v1.0.

---

## 14. References

- Kulkarni et al., *Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases*, 2014.
- Shapiro et al., *Conflict-free Replicated Data Types*, INRIA RR-7687, 2011.
- IETF RFC 8949 (CBOR), RFC 7518 (JOSE), RFC 7748 (X25519).
- Frank Denis, *libsodium* documentation.
