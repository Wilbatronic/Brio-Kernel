use brio_kernel::host::BrioHostState;
use brio_kernel::infrastructure::{audit, config::Settings, server, telemetry::TelemetryBuilder};
use secrecy::ExposeSecret;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Load Configuration
    // We do this first because telemetry might need config (e.g. OTLP endpoint)
    let config = Settings::new().expect("Failed to load configuration");

    // 2. Initialize Telemetry (Logging, Tracing, Metrics)
    let mut telemetry_builder = TelemetryBuilder::new("brio-kernel", "0.1.0")
        .with_log_level("debug")
        .with_sampling_ratio(config.telemetry.sampling_ratio);

    // Wire up OTLP if endpoint is present
    telemetry_builder = if let Some(ref endpoint) = config.telemetry.otlp_endpoint {
        telemetry_builder.with_tracing(endpoint)
    } else {
        telemetry_builder
    };

    // Initialize
    telemetry_builder
        .with_metrics()
        .init()
        .expect("Failed to initialize telemetry");

    info!("Brio Kernel Starting...");
    audit::log_audit(audit::AuditEvent::SystemStartup {
        component: "Kernel".into(),
    });

    // 3. Start Control Plane (Background Task)
    // We clone config to pass ownership to the background task (Settings is cheap to clone usually, or wrap in Arc)
    let server_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = server::run_server(&server_config).await {
            error!("Control Plane failed: {:?}", e);
        }
    });

    // 4. Initialize Host State (The Core Domain)
    let db_url = config.database.url.expose_secret();
    let _state = match BrioHostState::new(db_url).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize host state: {:?}", e);
            // We exit here because the kernel cannot function without state
            std::process::exit(1);
        }
    };

    info!("Brio Kernel Initialized. Waiting for shutdown signal...");

    // 5. Wait for Shutdown
    shutdown_signal().await;

    info!("Shutdown signal received, cleaning up...");
    audit::log_audit(audit::AuditEvent::SystemShutdown {
        reason: "Signal received".into(),
    });

    // Explicit cleanup if necessary

    info!("Brio Kernel Shutdown Complete.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
