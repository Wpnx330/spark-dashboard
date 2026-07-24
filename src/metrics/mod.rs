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
#[derive(Clone, serde::Serialize, Debug)]
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
) {
    let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));

    // Persistent sysinfo instances for delta-based metrics
    let mut sys = sysinfo::System::new();
    let mut networks = sysinfo::Networks::new_with_refreshed_list();
    let mut disks = sysinfo::Disks::new_with_refreshed_list();

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
                        // Sum power across ALL GPUs (TP1=1x, TP2=2x, TP4=4x, etc.)
                        // Falls back to the primary GPU when the full list is empty.
                        if snapshot.gpus.is_empty() {
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
                        },
                        snapshot.gpu.utilization_percent.map(|v| v as f64),
                        snapshot.gpu.temperature_celsius.map(|v| v as f64),
                        m.active_requests.map(|v| v as i64),
                        m.queued_requests.map(|v| v as i64),
                        m.kv_cache_percent,
                        m.prefix_cache_hit_rate,
                        Some(snapshot.cpu.aggregate_percent as f64),
                        None, // mem_used_pct - not directly available
                    )
                    .await
                    .ok();
            }
        }
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
#[derive(Clone, serde::Serialize, Debug)]
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
#[derive(Clone, serde::Serialize, Debug)]
pub struct CpuMetrics {
    pub name: Option<String>,
    pub aggregate_percent: f32,
    pub per_core: Vec<CoreMetrics>,
}

/// Per-core CPU usage.
#[derive(Clone, serde::Serialize, Debug)]
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
#[derive(Clone, serde::Serialize, Debug)]
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
#[derive(Clone, serde::Serialize, Debug)]
pub struct DiskMetrics {
    pub name: Option<String>,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
}

/// Network I/O throughput rates.
#[derive(Clone, serde::Serialize, Debug)]
pub struct NetworkMetrics {
    pub name: Option<String>,
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
}
