/// The service an XSTS ticket is scoped to.
///
/// Xbox Live issues distinct XSTS tickets per relying party — a ticket minted for Xbox Live
/// itself cannot be used against a third-party service's API, and vice versa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelyingParty {
    /// Xbox Live itself (e.g. the people-search/profile APIs).
    Xbox,
    /// Halo Waypoint (`prod.xsts.halowaypoint.com`), used to mint a Halo "spartan token".
    Halo,
}

impl RelyingParty {
    /// The relying-party URI Xbox Live's XSTS endpoint expects.
    pub fn as_str(&self) -> &'static str {
        match self {
            RelyingParty::Xbox => "http://xboxlive.com",
            RelyingParty::Halo => "https://prod.xsts.halowaypoint.com/",
        }
    }
}
