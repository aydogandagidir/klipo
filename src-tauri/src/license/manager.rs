//! License + trial state manager.
//!
//! Storage shape (rows in the `settings` k/v table; values are strings):
//!   - `trial_started_at`           — i64 unix-ms; first launch timestamp.
//!   - `license_key`                — Gumroad license key (raw).
//!   - `license_email`              — purchaser email.
//!   - `license_product_name`       — Gumroad product name (display).
//!   - `license_purchase_id`        — Gumroad purchase / sale id.
//!   - `license_activated_at`       — i64 unix-ms; first activation.
//!   - `license_last_verified_at`   — i64 unix-ms; latest successful verify.
//!   - `license_grace_until`        — i64 unix-ms; offline grace expiry.
//!
//! These keys live alongside the existing user settings (theme, hotkey, …)
//! so we don't need a separate migration. They're whitelisted in
//! `commands.rs::is_known_setting`, but the renderer can NOT read them
//! directly — `get_setting` is gated; license commands read on its behalf.
//!
//! **Security note:** All enforcement is client-side. A determined attacker
//! can patch the binary so `capture_allowed` always returns true. This is
//! an honest-user deterrent, identical in spirit to the `wa-contact`
//! extension's license-manager.js. The 7-day re-verify cadence + 30-day
//! grace window is the same compromise: closes the refund-abuse loop while
//! protecting honest users on bad networks.

use serde::{Deserialize, Serialize};

use crate::license::{
    gumroad::{verify_license, GumroadError, VerifyResult},
    GRACE_DAYS, KLIPO_PRODUCT_ID_DEFAULT, PRODUCT_ID_OVERRIDE_KEY, REVERIFY_AFTER_DAYS, TRIAL_DAYS,
};
use crate::storage::Storage;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error("storage: {0}")]
    Storage(#[from] crate::storage::error::StorageError),

    #[error("gumroad: {0}")]
    Gumroad(#[from] GumroadError),

    #[error("license key is required")]
    MissingKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseTier {
    Free,
    Pro,
    Trial,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub tier: LicenseTier,
    /// Stable string discriminant the UI may key on without losing fidelity:
    /// `no-license`, `verified`, `grace`, `expired-grace`, `trial-active`,
    /// `trial-expired`.
    pub reason: String,
    pub email: Option<String>,
    pub product_name: Option<String>,
    pub activated_at: Option<i64>,
    pub last_verified_at: Option<i64>,
    pub grace_until: Option<i64>,
    /// First / last 4 chars of the key with `…` between. Sent to the UI so
    /// the user can tell which key is active without us exposing the full
    /// secret to a potentially-compromised renderer.
    pub key_masked: Option<String>,
    /// Days until trial expiry. Some only when `tier == Trial`. Negative
    /// values are clamped to 0; expiry sets `tier = Expired` instead.
    pub trial_days_remaining: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialStatus {
    pub days_remaining: i64,
    pub expired: bool,
    pub started_at: i64,
}

/// Result of a periodic re-verify call. Mirrors the JS reference's
/// `{ ok, kind }` discriminated union but with typed payloads so the IPC
/// layer can pass it back to the renderer without stringifying.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReverifyOutcome {
    /// Fresh successful verify. Stored state was extended; status is the
    /// new computed value.
    Verified { status: LicenseStatus },
    /// Gumroad reports the key is unknown — usually means the user typed
    /// the wrong key or the listing was deleted. License has been cleared.
    Invalid { message: String },
    /// Refund / chargeback / disputed sale. License has been cleared.
    Refunded { message: String },
    /// Transport failure. License is *not* cleared; offline grace applies.
    Network { message: String },
    /// 5xx, garbled JSON, anything else. Same posture as `Network`.
    Server { message: String },
    /// Nothing to verify (no license stored).
    NoLicense,
}

// ---------------- Public API ----------------

/// Compute the current trial status, initialising `trial_started_at` on
/// the first call. Idempotent.
pub async fn get_trial_status(storage: &Storage) -> TrialStatus {
    let started_at = match storage.get_setting("trial_started_at").await {
        Ok(Some(s)) => s.parse::<i64>().ok(),
        _ => None,
    };
    let started_at = match started_at {
        Some(v) if v > 0 => v,
        _ => {
            // First-ever call → stamp the trial start. Best-effort: if the
            // write fails (read-only filesystem, etc.) we still return a
            // reasonable status, but the user will get a fresh trial again
            // on the next launch. That's a worse-than-ideal outcome but not
            // a security regression.
            let now = now_ms();
            let _ = storage
                .set_setting("trial_started_at", &now.to_string())
                .await;
            now
        }
    };

    let elapsed_ms = now_ms() - started_at;
    let trial_window_ms = TRIAL_DAYS * DAY_MS;
    let remaining_ms = trial_window_ms - elapsed_ms;
    // Ceil-divide so the user sees "14 days left" the moment they install
    // (instead of "13" because two milliseconds passed between the stamp
    // and the read), and so the badge counts down as 14 → 13 → … → 1 → 0.
    // `0` only when remaining_ms <= 0 (trial expired).
    let days_remaining = if remaining_ms <= 0 {
        0
    } else {
        (remaining_ms + DAY_MS - 1) / DAY_MS
    };
    let expired = remaining_ms <= 0;

    TrialStatus {
        days_remaining,
        expired,
        started_at,
    }
}

/// Compute the full license + trial status the UI renders.
///
/// Resolution order:
///   1. License key + verified within `GRACE_DAYS` → `Pro` / verified.
///   2. License key + within `grace_until` → `Pro` / grace.
///   3. License key but past grace → `Free` / expired-grace.
///   4. No license + trial active → `Trial`.
///   5. No license + trial expired → `Expired`.
pub async fn get_status(storage: &Storage) -> LicenseStatus {
    let key = storage_str(storage, "license_key").await;

    if let Some(key) = key.as_deref().filter(|k| !k.is_empty()) {
        let email = storage_str(storage, "license_email").await;
        let product_name = storage_str(storage, "license_product_name").await;
        let activated_at = storage_i64(storage, "license_activated_at").await;
        let last_verified_at = storage_i64(storage, "license_last_verified_at").await;
        let grace_until = storage_i64(storage, "license_grace_until").await;
        let now = now_ms();

        let since_verify = last_verified_at.map(|t| now - t).unwrap_or(i64::MAX);
        let grace_window = GRACE_DAYS * DAY_MS;

        if since_verify < grace_window {
            return LicenseStatus {
                tier: LicenseTier::Pro,
                reason: "verified".to_string(),
                email,
                product_name,
                activated_at,
                last_verified_at,
                grace_until,
                key_masked: Some(mask_key(key)),
                trial_days_remaining: None,
            };
        }
        if let Some(g) = grace_until {
            if now < g {
                return LicenseStatus {
                    tier: LicenseTier::Pro,
                    reason: "grace".to_string(),
                    email,
                    product_name,
                    activated_at,
                    last_verified_at,
                    grace_until,
                    key_masked: Some(mask_key(key)),
                    trial_days_remaining: None,
                };
            }
        }
        // License exists but neither verify-window nor grace-window holds.
        // Fall through to "expired-grace" — UI prompts the user to re-check.
        return LicenseStatus {
            tier: LicenseTier::Free,
            reason: "expired-grace".to_string(),
            email,
            product_name,
            activated_at,
            last_verified_at,
            grace_until,
            key_masked: Some(mask_key(key)),
            trial_days_remaining: None,
        };
    }

    // No license stored. Fall back to the trial.
    let trial = get_trial_status(storage).await;
    if trial.expired {
        LicenseStatus {
            tier: LicenseTier::Expired,
            reason: "trial-expired".to_string(),
            email: None,
            product_name: None,
            activated_at: None,
            last_verified_at: None,
            grace_until: None,
            key_masked: None,
            trial_days_remaining: Some(0),
        }
    } else {
        LicenseStatus {
            tier: LicenseTier::Trial,
            reason: "trial-active".to_string(),
            email: None,
            product_name: None,
            activated_at: None,
            last_verified_at: None,
            grace_until: None,
            key_masked: None,
            trial_days_remaining: Some(trial.days_remaining),
        }
    }
}

/// True when the clipboard pipeline should record clips. Honored by
/// `pipeline.rs` — when this returns false, captures are silently dropped
/// (no DB row, no blob, no FTS work). Not gated on telemetry-style
/// settings; we only block capture when the user has neither an active
/// license nor an active trial.
pub async fn capture_allowed(storage: &Storage) -> bool {
    let status = get_status(storage).await;
    matches!(status.tier, LicenseTier::Pro | LicenseTier::Trial)
}

/// First-time activation. Increments the Gumroad uses counter so each
/// device counts against the user's 3-device allowance.
///
/// On success, persists the full license metadata + a fresh 30-day grace
/// window and returns the resulting status.
pub async fn activate(
    storage: &Storage,
    key: &str,
    email_override: Option<&str>,
) -> Result<LicenseStatus, ManagerError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(ManagerError::MissingKey);
    }
    let product_id = resolve_product_id(storage).await;

    // Online verify + count this device.
    let result = verify_license(&product_id, trimmed, true).await?;

    let now = now_ms();
    write_license(
        storage,
        trimmed,
        &result,
        email_override.map(str::to_string),
        now,
        now,
        now + GRACE_DAYS * DAY_MS,
    )
    .await?;

    Ok(get_status(storage).await)
}

/// Background re-check. Does NOT count a device. Updates `last_verified_at`
/// and extends `grace_until` on success. Clears the license on
/// `Invalid`/`Refunded`. Leaves the license intact on `Network`/`Server`.
pub async fn reverify(storage: &Storage) -> ReverifyOutcome {
    let key = match storage_str(storage, "license_key").await {
        Some(k) if !k.is_empty() => k,
        _ => return ReverifyOutcome::NoLicense,
    };
    let product_id = resolve_product_id(storage).await;

    match verify_license(&product_id, &key, false).await {
        Ok(result) => {
            // Preserve the original activation timestamp; only bump the
            // verify+grace fields.
            let activated_at = storage_i64(storage, "license_activated_at")
                .await
                .unwrap_or_else(now_ms);
            let now = now_ms();
            if let Err(e) = write_license(
                storage,
                &key,
                &result,
                None,
                activated_at,
                now,
                now + GRACE_DAYS * DAY_MS,
            )
            .await
            {
                tracing::warn!(
                    target: "klipo::license",
                    error = %e,
                    "reverify succeeded but persisting state failed"
                );
            }
            ReverifyOutcome::Verified {
                status: get_status(storage).await,
            }
        }
        Err(GumroadError::Invalid(msg)) => {
            // Hard-clear: the key is bad. Closes refund-abuse / typo / leaked-key.
            let _ = clear_license(storage).await;
            ReverifyOutcome::Invalid { message: msg }
        }
        Err(GumroadError::Refunded(msg)) => {
            let _ = clear_license(storage).await;
            ReverifyOutcome::Refunded { message: msg }
        }
        Err(GumroadError::Network(msg)) => ReverifyOutcome::Network { message: msg },
        Err(GumroadError::Server(msg)) => ReverifyOutcome::Server { message: msg },
    }
}

/// User-initiated deactivation (Settings → License → Deactivate). Wipes
/// every license_* setting key so the user falls back to the trial / free
/// posture cleanly.
pub async fn deactivate(storage: &Storage) -> Result<(), ManagerError> {
    clear_license(storage).await
}

/// Fire a re-verify if `last_verified_at` was longer than `REVERIFY_AFTER_DAYS`
/// ago. Best-effort, never panics — designed to be spawned from the Tauri
/// `setup` hook. Does nothing when no license is present.
pub async fn maybe_reverify_on_startup(storage: &Storage) {
    let last_verified = storage_i64(storage, "license_last_verified_at").await;
    let key = storage_str(storage, "license_key").await;
    let Some(_) = key.as_deref().filter(|k| !k.is_empty()) else {
        return;
    };
    let due = match last_verified {
        Some(t) => (now_ms() - t) > REVERIFY_AFTER_DAYS * DAY_MS,
        None => true,
    };
    if !due {
        return;
    }
    tracing::info!(
        target: "klipo::license",
        "periodic re-verify due; calling Gumroad"
    );
    let outcome = reverify(storage).await;
    match outcome {
        ReverifyOutcome::Verified { .. } => {
            tracing::info!(target: "klipo::license", "re-verify ok");
        }
        ReverifyOutcome::Invalid { .. } | ReverifyOutcome::Refunded { .. } => {
            tracing::warn!(
                target: "klipo::license",
                "re-verify returned invalid/refunded; license cleared"
            );
        }
        ReverifyOutcome::Network { .. } | ReverifyOutcome::Server { .. } => {
            tracing::info!(
                target: "klipo::license",
                "re-verify failed transiently; relying on grace"
            );
        }
        ReverifyOutcome::NoLicense => {}
    }
}

// ---------------- Internals ----------------

async fn resolve_product_id(storage: &Storage) -> String {
    storage_str(storage, PRODUCT_ID_OVERRIDE_KEY)
        .await
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| KLIPO_PRODUCT_ID_DEFAULT.to_string())
}

async fn write_license(
    storage: &Storage,
    key: &str,
    result: &VerifyResult,
    email_override: Option<String>,
    activated_at: i64,
    last_verified_at: i64,
    grace_until: i64,
) -> Result<(), ManagerError> {
    storage.set_setting("license_key", key).await?;

    let email = email_override
        .or_else(|| result.email.clone())
        .unwrap_or_default();
    storage.set_setting("license_email", &email).await?;

    if let Some(name) = &result.product_name {
        storage.set_setting("license_product_name", name).await?;
    }
    if let Some(pid) = &result.purchase_id {
        storage.set_setting("license_purchase_id", pid).await?;
    }
    storage
        .set_setting("license_activated_at", &activated_at.to_string())
        .await?;
    storage
        .set_setting("license_last_verified_at", &last_verified_at.to_string())
        .await?;
    storage
        .set_setting("license_grace_until", &grace_until.to_string())
        .await?;
    Ok(())
}

async fn clear_license(storage: &Storage) -> Result<(), ManagerError> {
    for key in [
        "license_key",
        "license_email",
        "license_product_name",
        "license_purchase_id",
        "license_activated_at",
        "license_last_verified_at",
        "license_grace_until",
    ] {
        storage.set_setting(key, "").await?;
    }
    Ok(())
}

async fn storage_str(storage: &Storage, key: &str) -> Option<String> {
    match storage.get_setting(key).await {
        Ok(Some(s)) if !s.is_empty() => Some(s),
        _ => None,
    }
}

async fn storage_i64(storage: &Storage, key: &str) -> Option<i64> {
    storage_str(storage, key).await.and_then(|s| s.parse().ok())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// "ABCD…WXYZ" — first / last 4 chars with an ellipsis between. Short keys
/// (≤ 8 chars) are shown in full, since masking would just duplicate them.
fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return key.to_string();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars.iter().skip(chars.len() - 4).collect();
    format!("{head}\u{2026}{tail}")
}

// ---------------- Tests ----------------

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh_storage() -> Storage {
        Storage::open_in_memory().await.expect("in-memory storage")
    }

    #[tokio::test]
    async fn trial_starts_on_first_call() {
        let s = fresh_storage().await;
        let before = s.get_setting("trial_started_at").await.unwrap();
        assert!(
            before.is_none(),
            "trial_started_at must be absent on a fresh DB"
        );

        let status = get_trial_status(&s).await;
        assert!(status.started_at > 0);
        assert_eq!(status.days_remaining, TRIAL_DAYS);
        assert!(!status.expired);

        let after = s.get_setting("trial_started_at").await.unwrap();
        assert!(after.is_some(), "first call must persist trial_started_at");
    }

    #[tokio::test]
    async fn trial_days_remaining_decreases_over_time() {
        let s = fresh_storage().await;
        // Set trial_started_at to 5 days ago.
        let five_days_ago = now_ms() - 5 * DAY_MS;
        s.set_setting("trial_started_at", &five_days_ago.to_string())
            .await
            .unwrap();

        let status = get_trial_status(&s).await;
        assert_eq!(status.days_remaining, 9, "5d in → 9 of 14 should remain");
        assert!(!status.expired);
    }

    #[tokio::test]
    async fn trial_expired_after_14_days() {
        let s = fresh_storage().await;
        let fifteen_days_ago = now_ms() - 15 * DAY_MS;
        s.set_setting("trial_started_at", &fifteen_days_ago.to_string())
            .await
            .unwrap();

        let status = get_trial_status(&s).await;
        assert!(status.expired, "trial older than 14 days must be expired");
        assert_eq!(status.days_remaining, 0);
    }

    #[tokio::test]
    async fn capture_allowed_during_trial() {
        let s = fresh_storage().await;
        // Initialise via getter so a default trial is stamped.
        let _ = get_trial_status(&s).await;
        assert!(capture_allowed(&s).await, "trial must allow capture");
    }

    #[tokio::test]
    async fn capture_blocked_after_trial_with_no_license() {
        let s = fresh_storage().await;
        let twenty_days_ago = now_ms() - 20 * DAY_MS;
        s.set_setting("trial_started_at", &twenty_days_ago.to_string())
            .await
            .unwrap();

        assert!(
            !capture_allowed(&s).await,
            "expired trial + no license must block capture"
        );
    }

    #[tokio::test]
    async fn capture_allowed_with_active_license() {
        let s = fresh_storage().await;
        // Trial expired.
        s.set_setting("trial_started_at", &(now_ms() - 30 * DAY_MS).to_string())
            .await
            .unwrap();
        // …but a freshly-verified license is on file.
        let now = now_ms();
        s.set_setting("license_key", "TEST-KEY-1234-5678")
            .await
            .unwrap();
        s.set_setting("license_email", "buyer@example.com")
            .await
            .unwrap();
        s.set_setting("license_activated_at", &now.to_string())
            .await
            .unwrap();
        s.set_setting("license_last_verified_at", &now.to_string())
            .await
            .unwrap();
        s.set_setting(
            "license_grace_until",
            &(now + GRACE_DAYS * DAY_MS).to_string(),
        )
        .await
        .unwrap();

        assert!(
            capture_allowed(&s).await,
            "valid license must allow capture even past trial expiry"
        );

        let status = get_status(&s).await;
        assert_eq!(status.tier, LicenseTier::Pro);
        assert_eq!(status.reason, "verified");
        assert!(status.key_masked.is_some());
    }

    #[tokio::test]
    async fn license_in_grace_window_keeps_pro() {
        let s = fresh_storage().await;
        // last_verified > GRACE_DAYS ago, but grace_until is still in the future.
        let now = now_ms();
        let long_ago = now - (GRACE_DAYS + 5) * DAY_MS;
        s.set_setting("license_key", "AB12-CD34-EF56-GH78")
            .await
            .unwrap();
        s.set_setting("license_activated_at", &long_ago.to_string())
            .await
            .unwrap();
        s.set_setting("license_last_verified_at", &long_ago.to_string())
            .await
            .unwrap();
        // Grace extends to 5 days from now.
        s.set_setting("license_grace_until", &(now + 5 * DAY_MS).to_string())
            .await
            .unwrap();

        let status = get_status(&s).await;
        assert_eq!(status.tier, LicenseTier::Pro);
        assert_eq!(status.reason, "grace");
        assert!(capture_allowed(&s).await);
    }

    #[tokio::test]
    async fn license_past_grace_falls_back_to_free() {
        let s = fresh_storage().await;
        let now = now_ms();
        let long_ago = now - 90 * DAY_MS;
        s.set_setting("license_key", "AB12-CD34-EF56-GH78")
            .await
            .unwrap();
        s.set_setting("license_last_verified_at", &long_ago.to_string())
            .await
            .unwrap();
        s.set_setting("license_grace_until", &(now - DAY_MS).to_string())
            .await
            .unwrap();
        // Old trial too.
        s.set_setting("trial_started_at", &(now - 30 * DAY_MS).to_string())
            .await
            .unwrap();

        let status = get_status(&s).await;
        assert_eq!(status.tier, LicenseTier::Free);
        assert_eq!(status.reason, "expired-grace");
        assert!(!capture_allowed(&s).await);
    }

    #[test]
    fn mask_key_obscures_middle() {
        assert_eq!(mask_key("ABCD-1234-5678-WXYZ"), "ABCD\u{2026}WXYZ");
        assert_eq!(mask_key("short"), "short");
    }

    #[tokio::test]
    async fn deactivate_clears_all_license_keys() {
        let s = fresh_storage().await;
        s.set_setting("license_key", "ABCD-1234").await.unwrap();
        s.set_setting("license_email", "a@b.c").await.unwrap();
        s.set_setting("license_last_verified_at", "1234")
            .await
            .unwrap();

        deactivate(&s).await.unwrap();

        // We persist empty strings (rather than DELETE rows) — `storage_str`
        // treats empty strings as absent, so the user falls back to free / trial.
        assert!(storage_str(&s, "license_key").await.is_none());
        assert!(storage_str(&s, "license_email").await.is_none());
        assert!(storage_str(&s, "license_last_verified_at").await.is_none());
    }
}
