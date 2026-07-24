mod cli;
mod engines;
mod history;
mod metrics;
mod server;
mod ws;

#[cfg(target_os = "linux")]
mod logs;

use clap::{Args, Parser, Subcommand};
use cli::service::ServiceCommand;
use engines::{ApiKeyResolver, EngineOverride, EngineType};
use server::AppState;
use std::process::ExitCode;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Spark Dashboard — Real-time hardware and LLM monitoring for Linux hosts with NVIDIA GPUs.
#[derive(Parser, Debug)]
#[command(name = "spark-dashboard", version, about)]
struct Cli {
    #[command(flatten)]
    run: RunArgs,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage the systemd service (install, uninstall, status).
    #[command(subcommand)]
    Service(ServiceCommand),
    /// Probe the local /healthz endpoint and exit 0 (healthy) or 1.
    ///
    /// Used as the container HEALTHCHECK: the distroless runtime has no shell or
    /// `wget`, so the image execs the binary itself to check liveness.
    Healthcheck(HealthcheckArgs),
}

#[derive(Args, Debug)]
struct HealthcheckArgs {
    /// Port the server listens on (probed over 127.0.0.1).
    #[arg(
        short = 'p',
        long,
        env = "SPARK_DASHBOARD_PORT",
        default_value_t = 3000
    )]
    port: u16,
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Port to listen on
    #[arg(
        short = 'p',
        long,
        env = "SPARK_DASHBOARD_PORT",
        default_value_t = 3000
    )]
    port: u16,

    /// Address to bind to
    #[arg(
        short = 'b',
        long,
        env = "SPARK_DASHBOARD_BIND",
        default_value = "0.0.0.0"
    )]
    bind: String,

    /// Metrics polling interval in milliseconds
    #[arg(long, env = "SPARK_DASHBOARD_POLL_INTERVAL", default_value_t = 1000)]
    poll_interval: u64,

    /// Optional NVML GPU index to monitor. By default, all available NVIDIA GPUs
    /// are monitored; set this to keep the dashboard focused on one device.
    /// Out-of-range values log a warning and fall back to empty GPU metrics.
    #[arg(long, env = "SPARK_DASHBOARD_GPU_INDEX")]
    gpu_index: Option<u32>,

    /// Number of fictive GPUs to append after the real ones (development aid).
    /// Each emits plausible oscillating metrics and occasional thermal events
    /// through the normal snapshot pipeline, so multi-GPU UI paths can be
    /// exercised on single-GPU (or GPU-less) hosts.
    #[arg(long, env = "SPARK_DASHBOARD_SIMULATE_GPUS", default_value_t = 0)]
    simulate_gpus: u32,

    /// Manually specify engine type (use with --engine-url)
    #[arg(long, value_name = "TYPE")]
    engine: Vec<String>,

    /// Manually specify engine endpoint URL (use with --engine)
    #[arg(long, value_name = "URL")]
    engine_url: Vec<String>,

    /// API key for an engine endpoint, paired by index with --engine-url.
    /// For auth-gated deployments (e.g. vLLM started with --api-key) this
    /// lets the initial /v1/models lookup succeed instead of 401-spamming.
    #[arg(long, value_name = "KEY")]
    engine_api_key: Vec<String>,

    /// Fallback API key applied to any engine endpoint without an explicit
    /// --engine-api-key (also covers auto-detected engines).
    #[arg(long, env = "SPARK_DASHBOARD_PROVIDER_API_KEY")]
    provider_api_key: Option<String>,

    /// Enable the experimental log viewer at /ws/logs (Linux only).
    ///
    /// When set, the dashboard streams container logs from the Docker daemon
    /// using the bollard API. This is opt-in because engine logs can contain
    /// prompts and request payloads; the dashboard binds 0.0.0.0 by default
    /// and /ws/logs is unauthenticated.
    #[cfg(target_os = "linux")]
    #[arg(
        long,
        env = "SPARK_DASHBOARD_ENABLE_LOG_VIEWER",
        default_value_t = false
    )]
    enable_log_viewer: bool,

    /// Enable the historical metrics API (`/api/history/*`) and recording.
    /// Off by default: the history endpoints include destructive (`prune`) and
    /// write (`settings`, `toggle`) operations that must not be exposed
    /// unauthenticated on `0.0.0.0`. Enable only on trusted networks.
    #[arg(long, env = "SPARK_DASHBOARD_ENABLE_HISTORY", default_value_t = false)]
    enable_history: bool,

    /// Path to the SQLite history database. If the path cannot be opened the
    /// dashboard falls back gracefully: it tries `/var/lib/spark-dashboard/`,
    /// then `/tmp/`, then `:memory:` — so a missing volume never crashes boot.
    /// Pass `:memory:` explicitly for an ephemeral DB (e.g. tests).
    #[arg(
        long,
        env = "SPARK_DASHBOARD_HISTORY_DB",
        default_value = "/data/history.db"
    )]
    history_db_path: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Command::Service(cmd)) => cli::service::dispatch(cmd),
        Some(Command::Healthcheck(args)) => return cli::healthcheck::run(args.port),
        None => run_server(cli.run),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_server(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async move { run_server_inner(args).await })
}

async fn run_server_inner(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Parse manual engine overrides: --engine ollama --engine-url http://localhost:11434
    // Both vectors must have the same length. Each pair creates an EngineOverride.
    let api_keys = ApiKeyResolver::from_pairs(
        &args.engine_url,
        &args.engine_api_key,
        args.provider_api_key.clone(),
    );

    let overrides: Vec<EngineOverride> = args
        .engine
        .iter()
        .zip(args.engine_url.iter())
        .filter_map(|(engine_str, url)| {
            let engine_type = match engine_str.to_lowercase().as_str() {
                "vllm" => EngineType::Vllm,
                unknown => {
                    tracing::warn!("Unknown engine type '{}', ignoring override", unknown);
                    return None;
                }
            };
            Some(EngineOverride {
                engine_type,
                endpoint: url.clone(),
                api_key: api_keys.resolve(url),
            })
        })
        .collect();

    if !overrides.is_empty() {
        tracing::info!("Manual engine overrides: {:?}", overrides);
    }

    let (tx, _rx) = broadcast::channel::<String>(16);

    // Shared engine state: engine collector writes, metrics collector reads
    let engine_state: Arc<RwLock<Vec<engines::EngineSnapshot>>> = Arc::new(RwLock::new(Vec::new()));

    // Initialize history database.
    //
    // We always open a database (even when history is disabled) so the
    // metrics collector has somewhere to no-op into and so the `--enable-history`
    // toggle can flip recording on at runtime without a restart. Recording is
    // gated by `HistoryDb::is_enabled()`, which defaults to false.
    //
    // Path resolution: if the caller passed an explicit `--history-db-path`
    // (other than the default), honor it. Otherwise try `/var/lib/spark-dashboard`
    // (if the directory exists and is writable), fall back to `/tmp`, and
    // finally to `:memory:` so the dashboard always starts — losing history on
    // restart is preferable to crashing on boot.
    let history_db = open_history_db(&args.history_db_path)?;

    if args.enable_history {
        tracing::warn!(
            "History API enabled: /api/history/* routes are registered. \
             These endpoints include destructive/write operations — only enable \
             on trusted networks."
        );
    }

    // Spawn engine collector loop as separate tokio task (Research Pitfall 7:
    // separate task so slow engine API calls don't block hardware metrics)
    tokio::spawn(engines::engine_collector_loop(
        engine_state.clone(),
        overrides,
        api_keys,
    ));

    // Pass engine_state and history_db to metrics collector so it includes
    // engines in snapshots and records per-engine history.
    let history_for_metrics = history_db.clone();
    tokio::spawn(metrics::metrics_collector(
        tx.clone(),
        args.poll_interval,
        args.gpu_index,
        args.simulate_gpus,
        engine_state.clone(),
        history_for_metrics,
    ));

    // Background task: roll up 1s→1h and 1h→1d every 30 minutes
    let history_for_rollup = history_db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
        loop {
            interval.tick().await;
            if let Err(e) = history_for_rollup.rollup_1s_to_1h().await {
                tracing::warn!("History 1s→1h rollup failed: {}", e);
            }
            if let Err(e) = history_for_rollup.rollup_1h_to_1d().await {
                tracing::warn!("History 1h→1d rollup failed: {}", e);
            }
        }
    });

    // Enable the log viewer if the opt-in flag was passed (Linux only).
    // This registers /ws/logs in the router; nothing is exposed by default.
    #[cfg(target_os = "linux")]
    if args.enable_log_viewer {
        logs::enable_log_viewer(engine_state.clone());
        tracing::info!(
            "Log viewer enabled at /ws/logs - unauthenticated, container logs are exposed"
        );
    }

    let app_state = Arc::new(AppState {
        tx,
        history: history_db,
    });

    let app = server::create_router(app_state, args.enable_history);

    let addr = format!("{}:{}", args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Spark Dashboard running at http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Open the history database, with graceful fallback.
///
/// Path resolution order:
/// 1. If `requested` is non-empty and not the default (`/data/history.db`),
///    honor it literally (caller asked for something specific).
/// 2. Try `/var/lib/spark-dashboard/history.db` if that directory exists and
///    is writable.
/// 3. Fall back to `/tmp/spark-dashboard-history.db` (always writable).
/// 4. Final fallback: `:memory:` with a warning — the dashboard stays up but
///    history is lost on restart.
fn open_history_db(requested: &str) -> Result<history::HistoryDb, Box<dyn std::error::Error>> {
    // If the caller passed an explicit non-default path, honor it directly.
    if !requested.is_empty() && requested != "/data/history.db" && requested != ":memory:" {
        match history::HistoryDb::open(requested) {
            Ok(db) => {
                tracing::info!("History database at {}", requested);
                return Ok(db);
            }
            Err(e) => {
                tracing::warn!(
                    "History database at {} failed ({}), falling back",
                    requested,
                    e
                );
            }
        }
    }

    if requested == ":memory:" {
        tracing::info!("History database at :memory: (explicit)");
        return Ok(history::HistoryDb::open(":memory:")?);
    }

    // Try /var/lib/spark-dashboard/history.db if the directory exists and is writable.
    let var_path = "/var/lib/spark-dashboard/history.db";
    let var_dir = std::path::Path::new("/var/lib/spark-dashboard");
    let use_var = var_dir.exists()
        && var_dir.is_dir()
        && !var_dir
            .metadata()
            .map(|m| m.permissions().readonly())
            .unwrap_or(true);
    if use_var {
        match history::HistoryDb::open(var_path) {
            Ok(db) => {
                tracing::info!("History database at {}", var_path);
                return Ok(db);
            }
            Err(e) => {
                tracing::warn!(
                    "History database at {} failed ({}), falling back to /tmp",
                    var_path,
                    e
                );
            }
        }
    }

    // Fall back to /tmp (always writable).
    let tmp_path = "/tmp/spark-dashboard-history.db";
    match history::HistoryDb::open(tmp_path) {
        Ok(db) => {
            tracing::info!("History database at {}", tmp_path);
            Ok(db)
        }
        Err(e) => {
            tracing::warn!(
                "History database at {} failed ({}), using :memory:",
                tmp_path,
                e
            );
            Ok(history::HistoryDb::open(":memory:")?)
        }
    }
}
