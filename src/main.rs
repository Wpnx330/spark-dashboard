mod cli;
mod engines;
mod history;
mod logs;
mod metrics;
mod server;
mod ws;

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

    /// NVML GPU index to monitor (0 = first GPU). On multi-GPU hosts, pick which
    /// device the dashboard reads. Out-of-range values log a warning and fall
    /// back to empty GPU metrics.
    #[arg(long, env = "SPARK_DASHBOARD_GPU_INDEX", default_value_t = 0)]
    gpu_index: u32,

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

    // Initialize history database
    // Use /tmp/ by default (always writable by the spark-dashboard system user).
    // Try /var/lib/ only if the directory already exists and is writable.
    let history_db = {
        let var_path = "/var/lib/spark-dashboard/history.db";
        let var_dir = std::path::Path::new("/var/lib/spark-dashboard");
        let use_var = var_dir.exists()
            && var_dir.is_dir()
            && !var_dir.metadata().map(|m| m.permissions().readonly()).unwrap_or(true);
        let path = if use_var {
            var_path
        } else {
            "/tmp/spark-dashboard-history.db"
        };
        match history::HistoryDb::open(path) {
            Ok(db) => {
                tracing::info!("History database at {}", path);
                db
            }
            Err(e) => {
                tracing::warn!("History database at {} failed ({}), using :memory:", path, e);
                history::HistoryDb::open(":memory:")?
            }
        }
    };

    // Spawn engine collector loop
    tokio::spawn(engines::engine_collector_loop(
        engine_state.clone(),
        overrides,
        api_keys,
    ));

    // Pass engine_state and history_db to metrics collector
    let history_for_metrics = history_db.clone();
    tokio::spawn(metrics::metrics_collector(
        tx.clone(),
        args.poll_interval,
        args.gpu_index,
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

    let app = server::create_router(app_state);

    let addr = format!("{}:{}", args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Spark Dashboard running at http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
