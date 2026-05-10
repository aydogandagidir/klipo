//! Klipo license + trial layer (M8).
//!
//! Two-tier entitlement model:
//!   - **Trial:** 14 days of full features starting at first launch. The
//!     popup shows a "Trial: N days left" footer; capture continues normally.
//!   - **Pro:** activated via a Gumroad license key. Online activation
//!     increments the Gumroad uses counter (per-device count → Gumroad
//!     enforces the 3-device limit on its side). 30-day offline grace
//!     window from the last successful re-verify.
//!
//! Periodic re-verify happens at app startup if `last_verified > 7 days`:
//!   - On `success` → extend grace window, refresh metadata.
//!   - On `invalid` / `refunded` / `chargebacked` / `disputed` → clear license
//!     immediately. This closes the refund-abuse loop within 7 days.
//!   - On `network` / `server` → keep the license, rely on the 30-day grace.
//!
//! **Security note:** all enforcement is client-side. A determined attacker
//! can patch the binary to always return `LicenseTier::Pro`. This is an
//! honest-user deterrent, the same trade-off the WA-contact extension makes.
//!
//! Re-exports:
//!   - `manager::*` for the high-level activate/reverify/status surface.
//!   - `gumroad::*` for the raw HTTP client (mostly internal).

pub mod gumroad;
pub mod manager;

pub use manager::{
    LicenseStatus, LicenseTier, ManagerError, ReverifyOutcome, TrialStatus,
};

/// Klipo's Gumroad product id. **TODO: replace after Gumroad listing is live.**
/// User can override at runtime via Settings → License → Advanced (dev only).
pub const KLIPO_PRODUCT_ID_DEFAULT: &str = "TODO_REPLACE_AFTER_GUMROAD_LISTING";

/// Trial length in days.
pub const TRIAL_DAYS: i64 = 14;

/// Offline grace window after a successful re-verify.
pub const GRACE_DAYS: i64 = 30;

/// Re-verify cadence — periodic check is triggered if last_verified was longer ago.
pub const REVERIFY_AFTER_DAYS: i64 = 7;

/// Public Gumroad / bluedev landing where the user buys the license.
pub const PURCHASE_URL: &str = "https://bluedev.dev/products/klipo";

/// Settings key under which the user-overridable Gumroad product id lives.
/// If unset, `KLIPO_PRODUCT_ID_DEFAULT` is used.
pub const PRODUCT_ID_OVERRIDE_KEY: &str = "license_product_id_override";
