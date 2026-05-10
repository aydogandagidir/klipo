//! Gumroad License API client.
//! Docs: https://help.gumroad.com/article/76-license-keys
//!
//! Single endpoint we use:
//!   POST https://api.gumroad.com/v2/licenses/verify
//!   Body (form-encoded): product_id, license_key, increment_uses_count
//!   Response: { success, uses, purchase: { email, sale_timestamp, refunded, ... } }
//!
//! We treat a license as VALID when:
//!   - HTTP 200
//!   - success === true
//!   - purchase.refunded === false
//!   - purchase.chargebacked is not true
//!   - purchase.disputed is not true (or dispute_won === true)
//!
//! Mirrors `wa-contact/src/license/gumroad-api.js` line-for-line. Same error
//! kinds (`network` / `invalid` / `refunded` / `server`) so the `manager`
//! layer can react identically.

use std::time::Duration;

use serde::Deserialize;

const VERIFY_URL: &str = "https://api.gumroad.com/v2/licenses/verify";
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum GumroadError {
    /// Transport-level failure: DNS, TCP, TLS, timeout. License layer should
    /// keep the existing entitlement and rely on the offline grace window.
    #[error("network error: {0}")]
    Network(String),

    /// HTTP 404 or `success=false` — the license key is unknown to Gumroad.
    /// License layer should clear the stored key.
    #[error("invalid license: {0}")]
    Invalid(String),

    /// `purchase.refunded` / `chargebacked` / `disputed` were true. License
    /// layer must clear the stored key (closes the refund-abuse loop).
    #[error("license refunded or disputed: {0}")]
    Refunded(String),

    /// Anything else: 5xx, garbled JSON, etc. Treat like `Network` — we
    /// don't know if the key is bad, so honour the grace window.
    #[error("server error: {0}")]
    Server(String),
}

impl GumroadError {
    /// Short string identifier matching the JS reference's `kind` field.
    /// Used by `manager::reverify` to map to a `ReverifyOutcome` variant
    /// and by IPC to surface a stable enum to the renderer.
    pub fn kind(&self) -> &'static str {
        match self {
            GumroadError::Network(_) => "network",
            GumroadError::Invalid(_) => "invalid",
            GumroadError::Refunded(_) => "refunded",
            GumroadError::Server(_) => "server",
        }
    }
}

/// Successful verify response, narrowed to the fields the manager needs.
#[derive(Debug, Clone, Default)]
pub struct VerifyResult {
    /// Gumroad's per-license activation counter. `None` if the response did
    /// not include it (older response shape) or the field was a non-number.
    pub uses: Option<i64>,
    pub email: Option<String>,
    pub sale_timestamp: Option<String>,
    pub product_name: Option<String>,
    pub purchase_id: Option<String>,
}

/// Issue a verify against Gumroad. Pure HTTP — no storage side effects.
///
/// Set `increment_uses_count = true` for the activation flow (counts a device
/// against the user's purchase). Set `false` for periodic re-verifies and
/// the manual "Re-check now" button so we don't burn through the user's
/// 3-device allowance every 7 days.
pub async fn verify_license(
    product_id: &str,
    license_key: &str,
    increment_uses_count: bool,
) -> Result<VerifyResult, GumroadError> {
    if product_id.is_empty() {
        return Err(GumroadError::Server(
            "Gumroad product_id is unset".to_string(),
        ));
    }
    let trimmed = license_key.trim();
    if trimmed.is_empty() {
        return Err(GumroadError::Invalid("license key is empty".to_string()));
    }

    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| GumroadError::Network(format!("client build: {e}")))?;

    let form = [
        ("product_id", product_id),
        ("license_key", trimmed),
        (
            "increment_uses_count",
            if increment_uses_count { "true" } else { "false" },
        ),
    ];

    let res = client
        .post(VERIFY_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .form(&form)
        .send()
        .await
        .map_err(|e| GumroadError::Network(e.to_string()))?;

    let status = res.status();
    let body_text = res
        .text()
        .await
        .map_err(|e| GumroadError::Server(format!("read body ({status}): {e}")))?;

    let parsed: VerifyResponse = match serde_json::from_str(&body_text) {
        Ok(v) => v,
        Err(_) => {
            return Err(GumroadError::Server(format!(
                "invalid JSON response ({status})"
            )));
        }
    };

    // 404 or success=false → key not found in Gumroad's database.
    if status.as_u16() == 404 || parsed.success == Some(false) {
        let msg = parsed
            .message
            .unwrap_or_else(|| "license key not found".to_string());
        return Err(GumroadError::Invalid(msg));
    }
    if !status.is_success() {
        return Err(GumroadError::Server(format!("HTTP {status}")));
    }

    // Verify the purchase is in good standing. Order matches the JS reference
    // for parity — refunded > chargebacked > disputed, and a successful
    // dispute_won flips disputed back to "valid".
    let purchase = parsed.purchase.unwrap_or_default();
    if purchase.refunded == Some(true) {
        return Err(GumroadError::Refunded(
            "this sale has been refunded — license is invalid".to_string(),
        ));
    }
    if purchase.chargebacked == Some(true) {
        return Err(GumroadError::Refunded(
            "payment was charged back — license is invalid".to_string(),
        ));
    }
    if purchase.disputed == Some(true) && purchase.dispute_won != Some(true) {
        return Err(GumroadError::Refunded(
            "payment is under dispute — license is temporarily suspended".to_string(),
        ));
    }

    Ok(VerifyResult {
        uses: parsed.uses,
        email: purchase.email,
        sale_timestamp: purchase.sale_timestamp,
        product_name: purchase.product_name,
        purchase_id: purchase.id.or(purchase.sale_id),
    })
}

// ---------------- Wire format ----------------

#[derive(Debug, Default, Deserialize)]
struct VerifyResponse {
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    uses: Option<i64>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    purchase: Option<PurchaseFields>,
}

#[derive(Debug, Default, Deserialize)]
struct PurchaseFields {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    sale_timestamp: Option<String>,
    #[serde(default)]
    product_name: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    sale_id: Option<String>,
    #[serde(default)]
    refunded: Option<bool>,
    #[serde(default)]
    chargebacked: Option<bool>,
    #[serde(default)]
    disputed: Option<bool>,
    #[serde(default)]
    dispute_won: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the request body shape we *would* send. We don't make a live
    /// network call (CI must not depend on Gumroad availability or burn the
    /// uses counter) — instead we hand-build the same form encoding via the
    /// reqwest `RequestBuilder::form` API used in production and assert each
    /// key/value pair shows up in the rendered URL-encoded body. If Gumroad
    /// ever drifts the contract, the live activate flow will surface the
    /// regression with a real `Invalid`/`Server` error.
    #[test]
    fn form_encoded_body_shape() {
        let product_id = "abcd1234";
        let license_key = "  XYZ-KEY  "; // intentional whitespace
        let increment = true;

        let form = [
            ("product_id", product_id),
            ("license_key", license_key.trim()),
            (
                "increment_uses_count",
                if increment { "true" } else { "false" },
            ),
        ];

        let client = reqwest::Client::new();
        let req = client
            .post(VERIFY_URL)
            .form(&form)
            .build()
            .expect("build request");

        let body = req.body().expect("body present");
        let body_bytes = body.as_bytes().expect("non-streaming body").to_vec();
        let encoded = String::from_utf8(body_bytes).expect("utf8 body");

        assert!(encoded.contains("product_id=abcd1234"), "body={encoded}");
        assert!(encoded.contains("license_key=XYZ-KEY"), "body={encoded}");
        assert!(
            encoded.contains("increment_uses_count=true"),
            "body={encoded}"
        );
        // No trailing whitespace leaked from the key.
        assert!(!encoded.contains("license_key=%20"), "body={encoded}");

        // And confirm the route is what we think it is.
        assert_eq!(
            req.url().as_str(),
            "https://api.gumroad.com/v2/licenses/verify"
        );
    }

    #[test]
    fn error_kinds_match_js_reference() {
        assert_eq!(GumroadError::Network("x".into()).kind(), "network");
        assert_eq!(GumroadError::Invalid("x".into()).kind(), "invalid");
        assert_eq!(GumroadError::Refunded("x".into()).kind(), "refunded");
        assert_eq!(GumroadError::Server("x".into()).kind(), "server");
    }

    #[test]
    fn empty_license_key_is_invalid() {
        // No HTTP call happens — early validation should reject this.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let res = rt.block_on(verify_license("prod", "   ", false));
        match res {
            Err(GumroadError::Invalid(_)) => {}
            other => panic!("expected Invalid, got {other:?}"),
        }
    }
}
