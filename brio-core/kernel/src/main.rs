use brio_kernel::host::BrioHostState;
use brio_kernel::infrastructure::{audit, config::Settings, server, telemetry::TelemetryBuilder};
use secrecy::ExposeSecret;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Settings::new().expect("Failed to load configuration");

    let mut telemetry_builder = TelemetryBuilder::new("brio-kernel", "0.1.0")
        .with_log_level("debug")
        .with_sampling_ratio(config.telemetry.sampling_ratio);

    telemetry_builder = if let Some(ref endpoint) = config.telemetry.otlp_endpoint {
        telemetry_builder.with_tracing(endpoint)
    } else {
        telemetry_builder
    };

    telemetry_builder
        .with_metrics()
        .init()
        .expect("Failed to initialize telemetry");

    info!("Brio Kernel Starting...");
    audit::log_audit(audit::AuditEvent::SystemStartup {
        component: "Kernel".into(),
    });

    let db_url = config.database.url.expose_secret();

    // Clean Code: Configure Provider (DIP)
    let provider_config = brio_kernel::inference::OpenAIConfig::new(
        secrecy::SecretString::new("sk-placeholder".into()), // Placeholder for now
        reqwest::Url::parse("https://openrouter.ai/api/v1/").expect("Invalid URL"),
    );
    let provider = brio_kernel::inference::OpenAIProvider::new(provider_config);
    
    // Create registry (common for both modes)
    let registry = brio_kernel::inference::ProviderRegistry::new();
    registry.register_arc("default", std::sync::Arc::new(provider));
    registry.set_default("default");

    // Check for distributed config
    let node_id = std::env::var("BRIO_NODE_ID").ok().map(brio_kernel::mesh::types::NodeId::from);
    let mesh_port = std::env::var("BRIO_MESH_PORT").ok().unwrap_or("50051".to_string());

    let state = if let Some(ref id) = node_id {
        info!("Initializing in Distributed Mode (Node ID: {})", id);
        match BrioHostState::new_distributed(db_url, registry, id.clone()).await {
            Ok(s) => std::sync::Arc::new(s),
            Err(e) => {
                error!("Failed to initialize distributed host state: {:?}", e);
                std::process::exit(1);
            }
        }
    } else {
        info!("Initializing in Standalone Mode");
        match BrioHostState::new(db_url, registry).await {
            Ok(s) => std::sync::Arc::new(s),
            Err(e) => {
                error!("Failed to initialize host state: {:?}", e);
                std::process::exit(1);
            }
        }
    };
    
    // Start gRPC server if distributed
    if let Some(id) = node_id {
        let state_clone = state.clone();
        let port = mesh_port.clone();
        tokio::spawn(async move {
             let addr = format!("0.0.0.0:{}", port).parse().expect("Invalid mesh address");
             let service = brio_kernel::mesh::service::MeshService::new(state_clone, id);
             
             info!("Mesh gRPC server listening on {}", addr);
             
             if let Err(e) = tonic::transport::Server::builder()
                .add_service(brio_kernel::mesh::grpc::mesh_transport_server::MeshTransportServer::new(service))
                .serve(addr)
                .await 
             {
                 error!("Mesh gRPC server failed: {:?}", e);
             }
        });
    }

    let broadcaster = state.broadcaster().clone();
    let server_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = server::run_server(&server_config, broadcaster).await {
            error!("Control Plane failed: {:?}", e);
        }
    });

    info!("Brio Kernel Initialized. Waiting for shutdown signal...");

    shutdown_signal().await;

    info!("Shutdown signal received, cleaning up...");
    audit::log_audit(audit::AuditEvent::SystemShutdown {
        reason: "Signal received".into(),
    });

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
