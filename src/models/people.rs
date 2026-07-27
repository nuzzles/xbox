use serde::Deserialize;

/// Response body from the Xbox Live people-search endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct PeopleSearchResponse {
    pub people: Vec<Person>,
}

/// A single result from the Xbox Live people-search endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct Person {
    pub xuid: String,
    pub gamertag: String,
}
