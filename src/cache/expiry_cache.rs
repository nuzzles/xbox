use std::future::Future;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::{Mutex, watch};

/// A cached value paired with the time at which it should be considered stale.
#[derive(Debug, Clone)]
pub struct CachedToken<T> {
    pub value: T,
    pub expires_at: DateTime<Utc>,
}

impl<T> CachedToken<T> {
    pub fn new(value: T, expires_at: DateTime<Utc>) -> Self {
        Self { value, expires_at }
    }
}

/// Buffer subtracted from `expires_at` when deciding whether a cached value is still usable,
/// to avoid handing out a token that expires mid-flight to the caller.
const EXPIRY_SKEW: Duration = Duration::minutes(1);

type FetchResult<T, E> = Result<CachedToken<T>, E>;

enum Slot<T, E> {
    Empty,
    Fetching(watch::Receiver<Option<FetchResult<T, E>>>),
    Ready(CachedToken<T>),
}

/// An expiry-aware, single-flight cache for one token/credential.
///
/// - Returns the cached value immediately if it is not within a small skew window of expiring.
/// - If the cache is empty or expired, exactly one concurrent caller performs the refresh
///   (via the closure passed to [`ExpiryTokenCache::get_or_refresh`]); all other concurrent
///   callers await that single in-flight refresh rather than triggering their own.
/// - [`ExpiryTokenCache::invalidate`] forces the next call to refresh, e.g. after an
///   authenticated request comes back 401.
pub struct ExpiryTokenCache<T, E> {
    slot: Mutex<Slot<T, E>>,
}

impl<T, E> Default for ExpiryTokenCache<T, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, E> ExpiryTokenCache<T, E> {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(Slot::Empty),
        }
    }
}

impl<T, E> ExpiryTokenCache<T, E>
where
    T: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    /// Returns a cached, non-expired value if one exists, otherwise fetches a new one via
    /// `generate`. Concurrent calls made while a fetch is in flight share that single fetch's
    /// result rather than each triggering their own.
    pub async fn get_or_refresh<F, Fut>(&self, mut generate: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = FetchResult<T, E>>,
    {
        loop {
            let mut receiver = {
                let mut guard = self.slot.lock().await;
                match &*guard {
                    Slot::Ready(cached) if !is_stale(cached) => return Ok(cached.value.clone()),
                    Slot::Fetching(rx) => rx.clone(),
                    _ => {
                        // We are the leader: claim the slot and perform the fetch ourselves.
                        let (tx, rx) = watch::channel(None);
                        *guard = Slot::Fetching(rx);
                        drop(guard);

                        let result = generate().await;

                        let mut guard = self.slot.lock().await;
                        *guard = match &result {
                            Ok(cached) => Slot::Ready(cached.clone()),
                            Err(_) => Slot::Empty,
                        };
                        drop(guard);

                        // Ignore send errors: if every follower already gave up (e.g. was
                        // cancelled), there's simply no one left to notify.
                        let _ = tx.send(Some(result.clone()));
                        return result.map(|cached| cached.value);
                    }
                }
            };

            // We are a follower: wait for the leader's result. If the leader's task is
            // cancelled before it finishes (dropping the sender without ever sending), loop
            // back around and race to become the new leader ourselves instead of hanging.
            loop {
                if let Some(result) = receiver.borrow().clone() {
                    return result.map(|cached| cached.value);
                }
                if receiver.changed().await.is_err() {
                    break;
                }
            }
        }
    }

    /// Returns the currently cached value without triggering a refresh, if one exists and is
    /// not expired.
    pub async fn get_existing(&self) -> Option<T> {
        let guard = self.slot.lock().await;
        match &*guard {
            Slot::Ready(cached) if !is_stale(cached) => Some(cached.value.clone()),
            _ => None,
        }
    }

    /// Forces the next call to `get_or_refresh` to fetch a new value, e.g. after the server
    /// rejects the cached credential.
    pub async fn invalidate(&self) {
        let mut guard = self.slot.lock().await;
        *guard = Slot::Empty;
    }
}

fn is_stale<T>(cached: &CachedToken<T>) -> bool {
    cached.expires_at <= Utc::now() + EXPIRY_SKEW
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestError(String);

    #[tokio::test]
    async fn returns_fresh_value_without_refetch() {
        let cache: ExpiryTokenCache<i32, TestError> = ExpiryTokenCache::new();
        let calls = Arc::new(AtomicUsize::new(0));

        let c = calls.clone();
        let v = cache
            .get_or_refresh(|| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(CachedToken::new(42, Utc::now() + Duration::hours(1)))
                }
            })
            .await
            .unwrap();
        assert_eq!(v, 42);

        let c = calls.clone();
        let v = cache
            .get_or_refresh(|| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(CachedToken::new(99, Utc::now() + Duration::hours(1)))
                }
            })
            .await
            .unwrap();
        assert_eq!(v, 42, "second call should return the cached value");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn refreshes_after_expiry() {
        let cache: ExpiryTokenCache<i32, TestError> = ExpiryTokenCache::new();

        cache
            .get_or_refresh(|| async { Ok(CachedToken::new(1, Utc::now() - Duration::seconds(1))) })
            .await
            .unwrap();

        let v = cache
            .get_or_refresh(|| async { Ok(CachedToken::new(2, Utc::now() + Duration::hours(1))) })
            .await
            .unwrap();
        assert_eq!(v, 2);
    }

    #[tokio::test]
    async fn invalidate_forces_refetch() {
        let cache: ExpiryTokenCache<i32, TestError> = ExpiryTokenCache::new();

        cache
            .get_or_refresh(|| async { Ok(CachedToken::new(1, Utc::now() + Duration::hours(1))) })
            .await
            .unwrap();

        cache.invalidate().await;

        let v = cache
            .get_or_refresh(|| async { Ok(CachedToken::new(2, Utc::now() + Duration::hours(1))) })
            .await
            .unwrap();
        assert_eq!(v, 2);
    }

    #[tokio::test]
    async fn concurrent_callers_single_flight() {
        let cache: Arc<ExpiryTokenCache<i32, TestError>> = Arc::new(ExpiryTokenCache::new());
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];
        for _ in 0..16 {
            let cache = cache.clone();
            let calls = calls.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_refresh(|| {
                        let calls = calls.clone();
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            // Simulate network latency so all callers pile up on this fetch.
                            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                            Ok(CachedToken::new(7, Utc::now() + Duration::hours(1)))
                        }
                    })
                    .await
            }));
        }

        for handle in handles {
            assert_eq!(handle.await.unwrap().unwrap(), 7);
        }

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "only one fetch should have run for all concurrent callers"
        );
    }

    #[tokio::test]
    async fn error_does_not_poison_the_cache() {
        let cache: ExpiryTokenCache<i32, TestError> = ExpiryTokenCache::new();

        let err = cache
            .get_or_refresh(|| async { Err::<CachedToken<i32>, _>(TestError("boom".into())) })
            .await
            .unwrap_err();
        assert_eq!(err, TestError("boom".into()));

        let v = cache
            .get_or_refresh(|| async { Ok(CachedToken::new(5, Utc::now() + Duration::hours(1))) })
            .await
            .unwrap();
        assert_eq!(v, 5);
    }

    #[tokio::test]
    async fn get_existing_returns_none_when_empty() {
        let cache: ExpiryTokenCache<i32, TestError> = ExpiryTokenCache::new();
        assert_eq!(cache.get_existing().await, None);
    }

    #[tokio::test]
    async fn get_existing_returns_cached_value() {
        let cache: ExpiryTokenCache<i32, TestError> = ExpiryTokenCache::new();
        cache
            .get_or_refresh(|| async { Ok(CachedToken::new(3, Utc::now() + Duration::hours(1))) })
            .await
            .unwrap();
        assert_eq!(cache.get_existing().await, Some(3));
    }
}
