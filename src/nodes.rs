use crate::metrics::MetricsSnapshot;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// A snapshot of a remote node's hardware metrics, polled from its
/// `/api/node/hw` endpoint by the main dashboard's node-polling loop.
///
/// When a node goes offline its `snapshot` is retained for a few polling
/// intervals so the UI can show the last known readings rather than blanks;
/// `online` flips to `false` immediately so the frontend can flag it.
#[derive(Clone, Debug, serde::Serialize)]
pub struct NodeSnapshot {
    pub hostname: String,
    pub url: String,
    pub online: bool,
    pub last_seen_ms: u64,
    pub snapshot: Option<MetricsSnapshot>,
}

/// Wire format returned by a node agent's `GET /api/node/hw` endpoint.
#[derive(Deserialize)]
struct NodeHwResponse {
    hostname: String,
    snapshot: MetricsSnapshot,
}

/// Shared, thread-safe collection of node snapshots.
pub type NodeSnapshots = Arc<RwLock<Vec<NodeSnapshot>>>;

/// Parse a comma-separated list of node URLs (e.g.
/// `http://192.168.10.188:3001,http://192.168.10.189:3001`) into a `Vec`.
pub fn parse_node_urls(s: &str) -> Vec<String> {
    s.split(',')
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .collect()
}

/// Spawn the background node-polling loop.
///
/// Every second this task fetches each node's `/api/node/hw` endpoint
/// concurrently, deserializes the response into a `NodeSnapshot`, and writes
/// the results into the shared `NodeSnapshots` collection. Nodes that fail to
/// respond are marked `online = false` but keep their last known `snapshot`
/// for a few polls so the UI degrades gracefully.
pub fn spawn_node_poller(urls: Vec<String>, snapshots: NodeSnapshots) {
    if urls.is_empty() {
        return;
    }
    tracing::info!("Polling {} remote node(s): {:?}", urls.len(), urls);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .user_agent(concat!("spark-dashboard/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to build HTTP client for node polling: {e}");
            return;
        }
    };

    // Number of polls to retain stale data before clearing (≈10 s at 1 Hz).
    const STALE_AFTER_POLLS: u32 = 10;

    tokio::spawn(async move {
        // Initialize slots so /api/nodes returns the full roster immediately.
        {
            let mut guard = snapshots.write().await;
            *guard = urls
                .iter()
                .map(|url| NodeSnapshot {
                    hostname: url.clone(),
                    url: url.clone(),
                    online: false,
                    last_seen_ms: 0,
                    snapshot: None,
                })
                .collect();
        }

        let mut interval = tokio::time::interval(Duration::from_secs(1));
        // Tracks consecutive failures per node URL for stale-data eviction.
        let mut fail_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        loop {
            interval.tick().await;

            // Poll all nodes concurrently.
            let futures: Vec<_> = urls
                .iter()
                .map(|url| {
                    let client = client.clone();
                    let url = url.clone();
                    async move {
                        let result: Result<NodeHwResponse, reqwest::Error> = async {
                            let resp = client
                                .get(format!("{url}/api/node/hw"))
                                .send()
                                .await?
                                .error_for_status()?;
                            resp.json::<NodeHwResponse>().await
                        }
                        .await;
                        (url, result)
                    }
                })
                .collect();

            let results = futures_util::future::join_all(futures).await;

            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let mut guard = snapshots.write().await;
            for (url, result) in results {
                let Some(slot) = guard.iter_mut().find(|n| n.url == url) else {
                    continue;
                };

                match result {
                    Ok(data) => {
                        slot.hostname = data.hostname;
                        slot.snapshot = Some(data.snapshot);
                        slot.online = true;
                        slot.last_seen_ms = now_ms;
                        fail_counts.insert(url.clone(), 0);
                    }
                    Err(e) => {
                        let count = fail_counts
                            .entry(url.clone())
                            .and_modify(|c| *c += 1)
                            .or_insert(1);
                        tracing::debug!("Node {url} poll failed (attempt {count}): {e}");
                        slot.online = false;
                        // After several consecutive failures, clear stale data.
                        if *count >= STALE_AFTER_POLLS {
                            slot.snapshot = None;
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_node_urls_splits_comma_list() {
        let urls = parse_node_urls("http://a:3001, http://b:3001 ,, http://c:3001");
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "http://a:3001");
        assert_eq!(urls[1], "http://b:3001");
        assert_eq!(urls[2], "http://c:3001");
    }

    #[test]
    fn parse_node_urls_empty_string() {
        assert!(parse_node_urls("").is_empty());
        assert!(parse_node_urls("  ,  ,  ").is_empty());
    }

    #[tokio::test]
    async fn spawn_node_poller_with_empty_urls_is_noop() {
        // Should not panic or spawn anything when given an empty URL list.
        let snapshots: NodeSnapshots = Arc::new(RwLock::new(Vec::new()));
        spawn_node_poller(Vec::new(), snapshots.clone());
        // The collection should remain empty.
        assert!(snapshots.read().await.is_empty());
    }
}
