pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod gpu_sim;
pub mod memory;
pub mod network;

use crate::engines::EngineSnapshot;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

/// A complete snapshot of all hardware metrics at a point in time.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct MetricsSnapshot {
    pub timestamp_ms: u64,
    /// Backwards-compatible primary GPU metric. Mirrors the first entry in
    /// `gpus`, or an empty metric when no GPU is available.
    pub gpu: GpuMetrics,
    /// Metrics for every monitored GPU. Empty when NVML is unavailable or the
    /// requested `--gpu-index` filter is out of range.
    pub gpus: Vec<GpuMetrics>,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub network: NetworkMetrics,
    pub engines: Vec<EngineSnapshot>,
    pub gpu_events: Vec<gpu::GpuEvent>,
}

/// Runs the metrics collection loop, broadcasting JSON snapshots to all subscribers.
///
/// This function is intended to be spawned as a background tokio task. It maintains
/// persistent sysinfo instances for accurate delta-based metrics (CPU, disk, network).
#[cfg(target_os = "linux")]
pub async fn metrics_collector(
    tx: broadcast::Sender<String>,
    poll_interval_ms: u64,
    gpu_index: Option<u32>,
    simulate_gpus: u32,
    engine_state: std::sync::Arc<tokio::sync::RwLock<Vec<EngineSnapshot>>>,
    history_db: crate::history::HistoryDb,
    node_snapshots: crate::nodes::NodeSnapshots,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));

    // Persistent sysinfo instances for delta-based metrics
    let mut sys = sysinfo::System::new();
    let mut networks = sysinfo::Networks::new_with_refreshed_list();
    let mut disks = sysinfo::Disks::new_with_refreshed_list();

    // Local hostname, used to keep this host's own agent (which may appear in
    // the node-poll list) out of the recorded cluster power total.
    let local_hostname: Option<String> = sysinfo::System::host_name();

    // Initialize NVML (gracefully handle absence)
    let nvml = nvml_wrapper::Nvml::init().ok();
    let devices = match nvml.as_ref() {
        Some(n) => {
            let count = n.device_count().unwrap_or(0);
            tracing::info!("NVML initialized: {} GPU(s) available", count);
            let indexes: Vec<u32> = match gpu_index {
                Some(index) if index >= count => {
                    tracing::warn!(
                        "--gpu-index {} is out of range (found {} GPU(s)); GPU metrics disabled",
                        index,
                        count
                    );
                    Vec::new()
                }
                Some(index) => vec![index],
                None => (0..count).collect(),
            };

            indexes
                .into_iter()
                .filter_map(|index| match n.device_by_index(index) {
                    Ok(device) => Some((index, device)),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to open GPU at index {}: {} - skipping device",
                            index,
                            e
                        );
                        None
                    }
                })
                .collect()
        }
        None => {
            tracing::warn!("NVML not available -- GPU metrics will be empty");
            Vec::new()
        }
    };
    let primary_device = devices.first().map(|(_, device)| device);

    // Fictive GPUs slot in after every device NVML reports (not just the
    // monitored ones), so their indices can never collide with real hardware.
    let simulated_base_index = nvml
        .as_ref()
        .and_then(|n| n.device_count().ok())
        .unwrap_or(0);
    if simulate_gpus > 0 {
        tracing::info!(
            "Simulating {} fictive GPU(s) at index {} and up (--simulate-gpus)",
            simulate_gpus,
            simulated_base_index
        );
    }

    // Initial CPU refresh (first reading will be 0%, second will be accurate)
    sys.refresh_cpu_usage();

    let mut memory_logged = false;
    // Track cumulative counter values to compute per-second deltas
    use std::collections::HashMap;
    let mut prev_prompt: HashMap<String, i64> = HashMap::new();
    let mut prev_gen: HashMap<String, i64> = HashMap::new();
    let mut prev_reqs: HashMap<String, i64> = HashMap::new();

    loop {
        interval.tick().await;

        // Refresh sysinfo state (MUST use same instances for deltas)
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        networks.refresh(true);
        disks.refresh(true);

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Read latest engine snapshots (non-blocking read from shared state)
        // and stamp each with the GPU(s) its PIDs are observed on. Computed
        // here rather than in the engine collector because this loop owns the
        // NVML device handles.
        let mut engines = engine_state.read().await.clone();
        let device_pids = gpu::collect_device_pids(&devices);
        for engine in &mut engines {
            engine.gpu_indexes = gpu::gpu_indexes_for_pids(&engine.pids, &device_pids);
        }

        let mut gpu_events = gpu::detect_gpu_events(&devices, timestamp_ms);
        gpu_events.extend(gpu_sim::simulated_gpu_events(
            simulate_gpus,
            simulated_base_index,
            timestamp_ms,
        ));

        let memory_metrics = memory::collect_memory_metrics(primary_device);
        if !memory_logged {
            tracing::info!(
                kernel_total_bytes = memory_metrics.total_bytes,
                nvml_total_bytes = ?memory_metrics.gpu_memory_total_bytes,
                display_total_bytes = memory_metrics.display_total_bytes,
                is_unified = memory_metrics.is_unified,
                "memory topology detected"
            );
            memory_logged = true;
        }

        let mut gpus = gpu::collect_gpu_metrics(&devices);
        gpus.extend(gpu_sim::simulated_gpus(
            simulate_gpus,
            simulated_base_index,
            timestamp_ms,
        ));
        let gpu = gpus.first().cloned().unwrap_or_else(gpu::empty_gpu_metrics);

        let engines_for_history = engines.clone();
        let snapshot = MetricsSnapshot {
            timestamp_ms,
            gpu,
            gpus,
            cpu: cpu::collect_cpu_metrics(&sys),
            memory: memory_metrics,
            disk: disk::collect_disk_metrics(&disks),
            network: network::collect_network_metrics(&networks),
            engines,
            gpu_events,
        };

        match serde_json::to_string(&snapshot) {
            Ok(json) => {
                // Ignore error -- means no receivers connected (normal during startup)
                let _ = tx.send(json);
            }
            Err(e) => {
                tracing::error!("Failed to serialize metrics: {}", e);
            }
        }

        // Log to history database (if enabled)
        let ts = timestamp_ms as i64;
        for eng in &engines_for_history {
            if let Some(m) = &eng.metrics {
                // Compute per-second deltas from cumulative counters
                let cur_prompt = m.total_prompt_tokens.map(|v| v as i64);
                let cur_gen = m.total_generation_tokens.map(|v| v as i64);
                let cur_reqs = m.total_requests.map(|v| v as i64);
                let delta_prompt = match (prev_prompt.get(&eng.endpoint), cur_prompt) {
                    (Some(&prev), Some(cur)) if cur >= prev => Some(cur - prev),
                    _ => None,
                };
                let delta_gen = match (prev_gen.get(&eng.endpoint), cur_gen) {
                    (Some(&prev), Some(cur)) if cur >= prev => Some(cur - prev),
                    _ => None,
                };
                let delta_reqs = match (prev_reqs.get(&eng.endpoint), cur_reqs) {
                    (Some(&prev), Some(cur)) if cur >= prev => Some(cur - prev),
                    _ => None,
                };
                if let Some(v) = cur_prompt {
                    prev_prompt.insert(eng.endpoint.clone(), v);
                }
                if let Some(v) = cur_gen {
                    prev_gen.insert(eng.endpoint.clone(), v);
                }
                if let Some(v) = cur_reqs {
                    prev_reqs.insert(eng.endpoint.clone(), v);
                }
                // Sum power across ALL local GPUs (TP1=1x, TP2=2x, TP4=4x, etc.)
                // Falls back to the primary GPU when the full list is empty.
                let local_power_watts: Option<f64> = if snapshot.gpus.is_empty() {
                    snapshot.gpu.power_watts
                } else {
                    Some(
                        snapshot
                            .gpus
                            .iter()
                            .filter_map(|g| g.power_watts)
                            .sum::<f64>(),
                    )
                    .filter(|v: &f64| *v > 0.0)
                    .or(snapshot.gpu.power_watts)
                };

                // Add power from every online remote node so the recorded
                // value reflects the *actual* total cluster draw rather than
                // just the local node. Falls back to local-only when no node
                // snapshots are available (single-node deployments). This
                // host's own agent is excluded by hostname so its GPUs are
                // not counted twice (once here via NVML, once via the poll
                // list) — see `cluster_power_watts`.
                let power_watts = cluster_power_watts(
                    local_power_watts,
                    &node_snapshots.read().await,
                    local_hostname.as_deref(),
                );
                history_db
                    .insert_1s(
                        &eng.endpoint,
                        ts,
                        delta_prompt,
                        delta_gen,
                        delta_reqs,
                        m.prompt_tokens_per_sec,
                        m.tokens_per_sec,
                        m.ttft_ms,
                        m.inter_token_latency_ms,
                        m.e2e_latency_ms,
                        power_watts,
                        snapshot.gpu.utilization_percent.map(|v| v as f64),
                        snapshot.gpu.temperature_celsius.map(|v| v as f64),
                        m.active_requests.map(|v| v as i64),
                        m.queued_requests.map(|v| v as i64),
                        m.kv_cache_percent,
                        m.prefix_cache_hit_rate,
                        Some(snapshot.cpu.aggregate_percent as f64),
                        None, // mem_used_pct - not directly available
                        m.preemptions_total.map(|v| v as i64),
                        m.queue_time_ms,
                        m.tpot_ms,
                        m.spec_decode_acceptance_rate,
                    )
                    .await
                    .ok();
            }
        }
    }
}

/// Combine local and remote GPU power readings into the recorded cluster
/// total (used for the history database, not the `/api/nodes` display path).
///
/// Sums `gpu.power_watts` across online nodes with a snapshot, then combines
/// that with the local node's own NVML reading. Nodes whose `hostname` matches
/// `local_hostname` (case-insensitive) are excluded from the remote sum: the
/// node-poll list may include this dashboard's own agent, and without the
/// exclusion the local GPUs would be counted twice. When `local_hostname` is
/// `None` (hostname lookup failed) no node is excluded — fail-open, so the
/// recorded total never undercounts.
#[cfg(target_os = "linux")]
fn cluster_power_watts(
    local_power_watts: Option<f64>,
    nodes: &[crate::nodes::NodeSnapshot],
    local_hostname: Option<&str>,
) -> Option<f64> {
    let remote_power_watts: f64 = nodes
        .iter()
        .filter(|n| n.online)
        .filter(|n| match local_hostname {
            Some(local) => !n.hostname.eq_ignore_ascii_case(local),
            None => true,
        })
        .filter_map(|n| n.snapshot.as_ref())
        .filter_map(|s| s.gpu.power_watts)
        .sum();
    match (local_power_watts, remote_power_watts) {
        (Some(local), remote) if remote > 0.0 => Some(local + remote),
        (Some(local), _) => Some(local),
        (None, remote) if remote > 0.0 => Some(remote),
        (None, _) => None,
    }
}

/// Non-Linux metrics collector stub for development.
#[cfg(not(target_os = "linux"))]
pub async fn metrics_collector(
    tx: broadcast::Sender<String>,
    poll_interval_ms: u64,
    _gpu_index: Option<u32>,
    simulate_gpus: u32,
    engine_state: std::sync::Arc<tokio::sync::RwLock<Vec<EngineSnapshot>>>,
    _history_db: crate::history::HistoryDb,
    _node_snapshots: crate::nodes::NodeSnapshots,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));

    // Persistent sysinfo instances for delta-based metrics
    let mut sys = sysinfo::System::new();
    let mut networks = sysinfo::Networks::new_with_refreshed_list();
    let mut disks = sysinfo::Disks::new_with_refreshed_list();

    tracing::warn!("Running on non-Linux platform -- GPU metrics will be stubs");

    // Initial CPU refresh
    sys.refresh_cpu_usage();

    loop {
        interval.tick().await;

        sys.refresh_cpu_usage();
        sys.refresh_memory();
        networks.refresh(true);
        disks.refresh(true);

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Read latest engine snapshots (non-blocking read from shared state)
        let engines = engine_state.read().await.clone();

        // The non-Linux stub always exposes one fake primary GPU at index 0,
        // so fictive GPUs start at index 1.
        let mut gpu_events = gpu::detect_gpu_events(timestamp_ms);
        gpu_events.extend(gpu_sim::simulated_gpu_events(
            simulate_gpus,
            1,
            timestamp_ms,
        ));

        let gpu = gpu::collect_gpu_metrics();
        let mut gpus = vec![gpu.clone()];
        gpus.extend(gpu_sim::simulated_gpus(simulate_gpus, 1, timestamp_ms));
        let snapshot = MetricsSnapshot {
            timestamp_ms,
            gpu,
            gpus,
            cpu: cpu::collect_cpu_metrics(&sys),
            memory: memory::collect_memory_metrics(&sys),
            disk: disk::collect_disk_metrics(&disks),
            network: network::collect_network_metrics(&networks),
            engines,
            gpu_events,
        };

        match serde_json::to_string(&snapshot) {
            Ok(json) => {
                let _ = tx.send(json);
            }
            Err(e) => {
                tracing::error!("Failed to serialize metrics: {}", e);
            }
        }
    }
}

/// GPU metrics collected via NVML.
/// Fields are `Option` because some queries may return `NotSupported` depending on the GPU.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct GpuMetrics {
    pub index: Option<u32>,
    pub name: Option<String>,
    pub utilization_percent: Option<u32>,
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub temperature_celsius: Option<u32>,
    pub power_watts: Option<f64>,
    pub power_limit_watts: Option<f64>,
    pub clock_graphics_mhz: Option<u32>,
    pub clock_sm_mhz: Option<u32>,
    pub clock_memory_mhz: Option<u32>,
    pub fan_speed_percent: Option<u32>,
}

/// CPU metrics with aggregate and per-core breakdown.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct CpuMetrics {
    pub name: Option<String>,
    pub aggregate_percent: f32,
    pub per_core: Vec<CoreMetrics>,
}

/// Per-core CPU usage.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct CoreMetrics {
    pub id: usize,
    pub usage_percent: f32,
}

/// Memory metrics. `is_unified` flags unified-memory systems (e.g. DGX Spark GB10,
/// GH200) where CPU and GPU share one pool; on discrete-GPU systems GPU VRAM is
/// reported separately via `gpu_memory_total_bytes` / `gpu_memory_used_bytes`.
///
/// `display_total_bytes` is the value the UI should show as the headline pool
/// size: on unified systems the kernel reserves a few GiB for firmware/GPU
/// carve-outs, so `total_bytes` (from `/proc/meminfo`) under-reports the
/// marketed capacity. NVML reports the full hardware-addressable unified pool,
/// so we prefer it when available. Used/available stay sourced from the kernel
/// view to keep utilisation percentages honest.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub display_total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub cached_bytes: u64,
    pub gpu_estimated_bytes: Option<u64>,
    pub gpu_memory_total_bytes: Option<u64>,
    pub gpu_memory_used_bytes: Option<u64>,
    pub is_unified: bool,
}

/// Disk I/O throughput rates.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct DiskMetrics {
    pub name: Option<String>,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
}

/// Network I/O throughput rates.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct NetworkMetrics {
    pub name: Option<String>,
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
}

#[cfg(all(test, target_os = "linux"))]
mod cluster_power_tests {
    use super::*;
    use crate::nodes::NodeSnapshot;

    /// Minimal snapshot with only the fields `cluster_power_watts` reads.
    fn snapshot_with_power(power_watts: Option<f64>) -> MetricsSnapshot {
        MetricsSnapshot {
            timestamp_ms: 0,
            gpu: GpuMetrics {
                index: Some(0),
                name: None,
                utilization_percent: None,
                memory_total_bytes: None,
                memory_used_bytes: None,
                temperature_celsius: None,
                power_watts,
                power_limit_watts: None,
                clock_graphics_mhz: None,
                clock_sm_mhz: None,
                clock_memory_mhz: None,
                fan_speed_percent: None,
            },
            gpus: Vec::new(),
            cpu: CpuMetrics {
                name: None,
                aggregate_percent: 0.0,
                per_core: Vec::new(),
            },
            memory: MemoryMetrics {
                total_bytes: 0,
                display_total_bytes: 0,
                used_bytes: 0,
                available_bytes: 0,
                cached_bytes: 0,
                gpu_estimated_bytes: None,
                gpu_memory_total_bytes: None,
                gpu_memory_used_bytes: None,
                is_unified: false,
            },
            disk: DiskMetrics {
                name: None,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
            },
            network: NetworkMetrics {
                name: None,
                rx_bytes_per_sec: 0,
                tx_bytes_per_sec: 0,
            },
            engines: Vec::new(),
            gpu_events: Vec::new(),
        }
    }

    fn node(hostname: &str, online: bool, power_watts: Option<f64>) -> NodeSnapshot {
        NodeSnapshot {
            hostname: hostname.to_string(),
            url: format!("http://{hostname}:3001"),
            online,
            last_seen_ms: 0,
            snapshot: Some(snapshot_with_power(power_watts)),
        }
    }

    #[test]
    fn cluster_power_combines_local_with_three_remote_nodes() {
        let nodes = vec![
            node("spark-4e38", true, Some(56.5)),
            node("spark-fc0d", true, Some(47.75)),
            node("spark-470c", true, Some(58.0)),
        ];
        let power = cluster_power_watts(Some(100.0), &nodes, Some("spark-4cac"));
        assert_eq!(power, Some(100.0 + 56.5 + 47.75 + 58.0));
    }

    #[test]
    fn cluster_power_excludes_remote_node_matching_local_hostname() {
        // Regression: the poll list includes this host's own agent, so one of
        // the "remote" nodes IS the local node. Its reading must not be added
        // on top of the local NVML value.
        let nodes = vec![
            node("spark-4cac", true, Some(56.5)),
            node("spark-4e38", true, Some(47.75)),
            node("spark-fc0d", true, Some(56.75)),
            node("spark-470c", true, Some(58.0)),
        ];
        let power = cluster_power_watts(Some(100.0), &nodes, Some("spark-4cac"));
        assert_eq!(power, Some(100.0 + 47.75 + 56.75 + 58.0));
        assert_ne!(
            power,
            Some(100.0 + 56.5 + 47.75 + 56.75 + 58.0),
            "local GPU power must not be double-counted"
        );
    }

    #[test]
    fn cluster_power_hostname_match_is_case_insensitive() {
        let nodes = vec![
            node("SPARK-4CAC", true, Some(56.5)),
            node("spark-4e38", true, Some(47.75)),
        ];
        let power = cluster_power_watts(Some(100.0), &nodes, Some("Spark-4cac"));
        assert_eq!(power, Some(100.0 + 47.75));
    }

    #[test]
    fn cluster_power_with_unknown_local_hostname_includes_all_remotes() {
        // Fail-open: without a hostname the local node cannot be identified,
        // so every online remote is summed (never undercount).
        let nodes = vec![
            node("spark-4cac", true, Some(56.5)),
            node("spark-4e38", true, Some(47.75)),
        ];
        let power = cluster_power_watts(Some(100.0), &nodes, None);
        assert_eq!(power, Some(100.0 + 56.5 + 47.75));
    }

    #[test]
    fn cluster_power_without_local_power_uses_remote_sum() {
        let nodes = vec![
            node("spark-4e38", true, Some(56.5)),
            node("spark-fc0d", true, Some(47.75)),
        ];
        let power = cluster_power_watts(None, &nodes, Some("spark-4cac"));
        assert_eq!(power, Some(56.5 + 47.75));
    }

    #[test]
    fn cluster_power_none_when_no_local_power_and_no_online_remote_power() {
        // Offline node with a stale snapshot and an online node without a
        // power reading both contribute nothing; with no local reading either
        // there is no power to record.
        let nodes = vec![
            node("spark-4e38", false, Some(56.5)),
            node("spark-fc0d", true, None),
        ];
        let power = cluster_power_watts(None, &nodes, Some("spark-4cac"));
        assert_eq!(power, None);
    }

    #[test]
    fn cluster_power_zero_local_reading_is_still_a_reading() {
        // Some(0.0) must not collapse to None when no remote power exists —
        // the local node is reporting, it is just idle.
        let nodes = vec![node("spark-4e38", false, Some(56.5))];
        assert_eq!(
            cluster_power_watts(Some(0.0), &nodes, Some("spark-4cac")),
            Some(0.0)
        );
    }

    #[test]
    fn cluster_power_adds_remotes_onto_zero_local_reading() {
        let nodes = vec![node("spark-4e38", true, Some(56.5))];
        assert_eq!(
            cluster_power_watts(Some(0.0), &nodes, Some("spark-4cac")),
            Some(56.5)
        );
    }
}
