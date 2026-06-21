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

pub use manager::{LicenseStatus, LicenseTier, ManagerError, ReverifyOutcome, TrialStatus};

/// Klipo's Gumroad product id, **as accepted by the `/v2/licenses/verify`
/// API** — a base64-encoded long form, not the short hash that appears in
/// the seller dashboard edit URL.
///
/// **Critical trap:** Gumroad exposes two distinct identifiers per product:
///
///   1. `short_product_id` — a 5-character hash like `hvdaw`. Shows up in
///      the edit URL (`gumroad.com/products/hvdaw/edit`). Tempting to copy
///      from the address bar. Used by Gumroad's own admin UI.
///   2. `product_id` — a base64-style long form like `kOmaM0_GJEzx5brZfBzHXA==`
///      (~24 chars, ends with `==` padding). **This** is what the licensing
///      API expects.
///
/// Sending the short form to `/v2/licenses/verify` is the default failure
/// mode (`{"success":false,"message":"That license does not exist for the
/// provided product."}`). Sending the wrong parameter name also fails, but
/// helpfully — Gumroad's response then leaks the correct long form:
///
///   $ curl -X POST https://api.gumroad.com/v2/licenses/verify \
///       -d "product_permalink=klipo&license_key=…"
///   {"success":false,"message":"The 'product_id' parameter is required
///    to verify the license for this product. Please set 'product_id' to
///    'kOmaM0_GJEzx5brZfBzHXA==' in the request."}
///
/// That's the diagnostic recipe — verify against the live product with
/// `product_permalink` instead of `product_id` and Gumroad will print the
/// canonical id back at you. Captured here on 2026-05-13 after the v0.1.7
/// hot-patch shipped with the wrong (short-hash) value.
///
/// The runtime override (`license_product_id_override` setting) stays
/// available for dev / staging verification against a different product.
pub const KLIPO_PRODUCT_ID_DEFAULT: &str = "kOmaM0_GJEzx5brZfBzHXA==";

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
