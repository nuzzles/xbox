use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// An Xbox Live XUID (unique player identifier), stored in its raw numeric-string form.
///
/// Use [`crate::util::xuid::wrap_xuid`]/[`crate::util::xuid::unwrap_xuid`] to convert to/from
/// the `xuid(123)` wrapped form used by several Xbox Live and Halo Waypoint endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Xuid(pub String);

impl Xuid {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Xuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Xuid {
    fn from(value: String) -> Self {
        Xuid(value)
    }
}

impl From<&str> for Xuid {
    fn from(value: &str) -> Self {
        Xuid(value.to_string())
    }
}

/// An Xbox Live "user token", obtained by exchanging an RPS ticket (or other credential)
/// against `user.auth.xboxlive.com/user/authenticate`.
#[derive(Debug, Clone, Deserialize)]
pub struct UserToken {
    #[serde(rename = "Token")]
    pub token: String,
    #[serde(rename = "NotAfter")]
    pub not_after: DateTime<Utc>,
}

/// The `DisplayClaims.xui[]` entry inside an XSTS ticket, carrying the user hash (`uhs`)
/// needed to build an `XBL3.0 x=<uhs>;<token>` authorization header, plus the gamertag/XUID of
/// the signed-in user themselves.
#[derive(Debug, Clone, Deserialize)]
pub struct XuiItem {
    pub uhs: String,
    /// The signed-in user's own gamertag. Present on Xbox Live relying-party tickets; may be
    /// absent for other relying parties.
    pub gtg: Option<String>,
    /// The signed-in user's own XUID. Present on Xbox Live relying-party tickets; may be
    /// absent for other relying parties.
    pub xid: Option<String>,
}

/// The `DisplayClaims` object inside an XSTS ticket.
#[derive(Debug, Clone, Deserialize)]
pub struct DisplayClaims {
    pub xui: Vec<XuiItem>,
}

/// An XSTS ticket, obtained by exchanging a user token for a specific relying party via
/// `xsts.auth.xboxlive.com/xsts/authorize`.
#[derive(Debug, Clone, Deserialize)]
pub struct XstsToken {
    #[serde(rename = "Token")]
    pub token: String,
    #[serde(rename = "NotAfter")]
    pub not_after: DateTime<Utc>,
    #[serde(rename = "DisplayClaims")]
    pub display_claims: DisplayClaims,
}

impl XstsToken {
    /// The user hash (`uhs`) from the first display-claims entry, needed to build an
    /// `XBL3.0 x=<uhs>;<token>` authorization header.
    pub fn user_hash(&self) -> Option<&str> {
        self.display_claims.xui.first().map(|xui| xui.uhs.as_str())
    }

    /// Builds the `XBL3.0 x=<uhs>;<token>` authorization header value for this ticket.
    pub fn authorization_header(&self) -> Option<String> {
        self.user_hash()
            .map(|uhs| format!("XBL3.0 x={uhs};{}", self.token))
    }

    /// The signed-in user's own gamertag, from the first display-claims entry.
    pub fn gamertag(&self) -> Option<&str> {
        self.display_claims
            .xui
            .first()
            .and_then(|xui| xui.gtg.as_deref())
    }

    /// The signed-in user's own XUID, from the first display-claims entry.
    pub fn xuid(&self) -> Option<Xuid> {
        self.display_claims
            .xui
            .first()
            .and_then(|xui| xui.xid.clone())
            .map(Xuid)
    }
}
