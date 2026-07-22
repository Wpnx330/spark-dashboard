mod cli;
mod engines;
mod history;
mod metrics;
mod server;
mod ws;

use clap::{Args, Parser, Subcommand};
use cli::service::ServiceCommand;
use engines::{ApiKeyResolver, EngineOverride, EngineType};
use server::AppState;
use std::path::Path;
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

    /// Enable the historical metrics API (`/api/history/*`) and recording.
    /// Off by default: the history endpoints include destructive (`prune`) and
    /// write (`settings`, `toggle`) operations that must not be exposed
    /// unauthenticated on `0.0.0.0`. Enable only on trusted networks.
    #[arg(long, env = "SPARK_DASHBOARD_ENABLE_HISTORY", default_value_t = false)]
    enable_history: bool,

    /// Path to the SQLite history database. The directory must exist and be
    /// writable by the running user. The process fails loudly if the file
    /// cannot be opened — there is no silent fallback.
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
    // Persistence: the path comes from `--history-db-path` /
    // `SPARK_DASHBOARD_HISTORY_DB` (default `/data/history.db`). We fail loudly
    // if the file cannot be opened — no silent fallback to `/tmp` (ephemeral
    // under containers) or `:memory:` (lost on restart). The parent directory
    // must exist and be writable by the running user; in the distroless
    // `nonroot` image (uid 65532) this means mounting a volume at `/data`
    // (see deploy/docker/docker-compose.yml).
    let history_db = open_history_db(Path::new(&args.history_db_path))?;

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

/// Open the history database, failing loudly on I/O errors.
///
/// There is deliberately no silent fallback to `/tmp` or `:memory:`: a silent
/// fallback would hide a misconfigured volume mount and lose data on restart.
/// If you need an ephemeral DB (e.g. for tests), pass `:memory:` explicitly
/// via `--history-db-path`.
fn open_history_db(path: &Path) -> Result<history::HistoryDb, Box<dyn std::error::Error>> {
    // For `:memory:` SQLite ignores the path, so skip the parent-dir check.
    if path != Path::new(":memory:") {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(format!(
                    "history database directory {} does not exist — create it and ensure it is \
                     writable by the running user (mount a volume at /data in containers; \
                     see deploy/docker/docker.md)",
                    parent.display()
                )
                .into());
            }
        }
    }
    match history::HistoryDb::open(&path.to_string_lossy()) {
        Ok(db) => {
            tracing::info!("History database at {}", path.display());
            Ok(db)
        }
        Err(e) => Err(format!(
            "failed to open history database at {}: {} — ensure the directory exists and is \
             writable (mount a volume at /data in containers)",
            path.display(),
            e
        )
        .into()),
    }
}
