use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

use dashmap::DashMap;

use super::expiry_cache::{CachedToken, ExpiryTokenCache};

/// A [`ExpiryTokenCache`] per key, for credentials that vary by some dimension (e.g. an XSTS
/// ticket per [`crate::auth::RelyingParty`], or a resolved XUID per gamertag).
pub struct KeyedExpiryTokenCache<K, T, E> {
    caches: DashMap<K, Arc<ExpiryTokenCache<T, E>>>,
}

impl<K, T, E> Default for KeyedExpiryTokenCache<K, T, E>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, T, E> KeyedExpiryTokenCache<K, T, E>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            caches: DashMap::new(),
        }
    }
}

impl<K, T, E> KeyedExpiryTokenCache<K, T, E>
where
    K: Eq + Hash + Clone,
    T: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    /// Returns a cached, non-expired value for `key` if one exists, otherwise fetches a new
    /// one via `generate`, single-flighted per key.
    pub async fn get_or_refresh<F, Fut>(&self, key: K, generate: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<CachedToken<T>, E>>,
    {
        // Clone the per-key cache's Arc out from under the DashMap shard lock, then drop the
        // guard before awaiting — otherwise we'd hold the shard lock (blocking any other key
        // hashed into the same shard) for the full duration of the fetch.
        let cache = self
            .caches
            .entry(key)
            .or_insert_with(|| Arc::new(ExpiryTokenCache::new()))
            .clone();
        cache.get_or_refresh(generate).await
    }

    /// Returns the currently cached value for `key`, if one exists and is not expired.
    pub async fn get_existing(&self, key: &K) -> Option<T> {
        let cache = self.caches.get(key).map(|entry| entry.clone());
        match cache {
            Some(cache) => cache.get_existing().await,
            None => None,
        }
    }

    /// Forces the next call to `get_or_refresh` for `key` to fetch a new value.
    pub async fn invalidate(&self, key: &K) {
        let cache = self.caches.get(key).map(|entry| entry.clone());
        if let Some(cache) = cache {
            cache.invalidate().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestError;

    #[tokio::test]
    async fn caches_independently_per_key() {
        let cache: KeyedExpiryTokenCache<&'static str, i32, TestError> =
            KeyedExpiryTokenCache::new();

        let a = cache
            .get_or_refresh("a", || async {
                Ok(CachedToken::new(1, Utc::now() + Duration::hours(1)))
            })
            .await
            .unwrap();
        let b = cache
            .get_or_refresh("b", || async {
                Ok(CachedToken::new(2, Utc::now() + Duration::hours(1)))
            })
            .await
            .unwrap();

        assert_eq!(a, 1);
        assert_eq!(b, 2);
    }

    #[tokio::test]
    async fn concurrent_different_keys_do_not_block_each_other() {
        let cache: Arc<KeyedExpiryTokenCache<u8, i32, TestError>> =
            Arc::new(KeyedExpiryTokenCache::new());
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];
        for key in 0..8u8 {
            let cache = cache.clone();
            let calls = calls.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_refresh(key, || {
                        let calls = calls.clone();
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Ok(CachedToken::new(
                                key as i32,
                                Utc::now() + Duration::hours(1),
                            ))
                        }
                    })
                    .await
            }));
        }

        for (i, handle) in handles.into_iter().enumerate() {
            assert_eq!(handle.await.unwrap().unwrap(), i as i32);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 8);
    }

    #[tokio::test]
    async fn invalidate_only_affects_one_key() {
        let cache: KeyedExpiryTokenCache<&'static str, i32, TestError> =
            KeyedExpiryTokenCache::new();

        cache
            .get_or_refresh("a", || async {
                Ok(CachedToken::new(1, Utc::now() + Duration::hours(1)))
            })
            .await
            .unwrap();
        cache
            .get_or_refresh("b", || async {
                Ok(CachedToken::new(2, Utc::now() + Duration::hours(1)))
            })
            .await
            .unwrap();

        cache.invalidate(&"a").await;

        assert_eq!(cache.get_existing(&"a").await, None);
        assert_eq!(cache.get_existing(&"b").await, Some(2));
    }
}
