use async_trait::async_trait;

use super::expiry_cache::CachedToken;

/// A pluggable backend for persisting a cached credential across process restarts.
///
/// The default [`InMemoryPersister`] is a no-op beyond the in-process [`super::ExpiryTokenCache`]
/// itself. A file-backed (or otherwise durable) persister is a natural v2 addition behind its
/// own feature flag, without requiring any change to this trait.
#[async_trait]
pub trait TokenPersister<T>: Send + Sync {
    async fn load(&self) -> Option<CachedToken<T>>;
    async fn save(&self, token: &CachedToken<T>);
    async fn clear(&self);
}

/// A [`TokenPersister`] that does not actually persist anything; every load returns `None`.
/// This is the default persister — relying purely on [`super::ExpiryTokenCache`]'s in-process
/// cache is sufficient for most callers, and adding real persistence is opt-in.
#[derive(Debug, Default)]
pub struct InMemoryPersister;

#[async_trait]
impl<T: Send + Sync> TokenPersister<T> for InMemoryPersister {
    async fn load(&self) -> Option<CachedToken<T>> {
        None
    }

    async fn save(&self, _token: &CachedToken<T>) {}

    async fn clear(&self) {}
}
