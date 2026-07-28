/// The service an XSTS ticket is scoped to.
///
/// Xbox Live issues distinct XSTS tickets per relying party — a ticket minted for Xbox Live
/// itself cannot be used against a third-party service's API, and vice versa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RelyingParty(&'static str);

impl RelyingParty {
    /// Xbox Live itself (e.g. the people-search/profile APIs).
    pub const XBOX: Self = Self("http://xboxlive.com");

    /// Creates a relying party owned by the downstream service that defines its URI.
    pub const fn new(uri: &'static str) -> Self {
        Self(uri)
    }

    /// The relying-party URI Xbox Live's XSTS endpoint expects.
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}
