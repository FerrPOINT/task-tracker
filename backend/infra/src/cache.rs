use std::time::Duration;

use moka::future::Cache as MokaCache;

#[cfg(test)]
#[path = "cache/tests.rs"]
mod tests;

#[derive(Clone)]
pub struct AppCache {
    inner: MokaCache<String, String>,
}

impl AppCache {
    pub fn new() -> Self {
        Self {
            inner: MokaCache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(300))
                .build(),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        self.inner.get(key).await
    }

    pub async fn set(&self, key: String, value: String) {
        self.inner.insert(key, value).await;
    }
}

impl Default for AppCache {
    fn default() -> Self {
        Self::new()
    }
}
