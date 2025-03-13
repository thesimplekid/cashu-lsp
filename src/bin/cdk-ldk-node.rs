use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, bail};
use bip39::Mnemonic;
use cdk::mint_url::MintUrl;
use cdk::nuts::CurrencyUnit;
use cdk::wallet::{MultiMintWallet, Wallet};
use cdk_ldk_node::config::AppConfig;
use cdk_ldk_node::db::Db;
use cdk_ldk_node::lsp_server::CashuLspInfo;
use cdk_ldk_node::proto::cdk_ldk_management_server::CdkLdkManagementServer;
use cdk_ldk_node::proto::server::CdkLdkServer;
use cdk_ldk_node::{BitcoinRpcConfig, ChainSource, GossipSource, create_cashu_lsp_router};
use ldk_node::lightning::ln::msgs::SocketAddress;
use tokio::signal;
use tonic::transport::Server;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let runtime = Arc::new(runtime);

    let runtime_clone = runtime.clone();

    runtime.block_on(async {
        let work_dir = home::home_dir()
            .ok_or(anyhow!("Could not get home dir"))?
            .join(".cashu-lsp");

        // Ensure work directory exists
        std::fs::create_dir_all(&work_dir)
            .map_err(|e| anyhow!("Failed to create work directory: {}", e))?;

        // Load configuration
        let config_path = work_dir.join("config.toml");
        let config = match AppConfig::new(Some(&config_path)) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to load configuration: {}", e);
                eprintln!(
                    "An example configuration has been created at: {}",
                    work_dir.join("example.config.toml").display()
                );
                eprintln!(
                    "Please copy and modify this file to: {}",
                    config_path.display()
                );
                return Err(anyhow::anyhow!("Configuration error: {}", e));
            }
        };

        let default_filter = "debug";
        let sqlx_filter = "sqlx=warn";
        let hyper_filter = "hyper=warn";
        let h2_filter = "h2=warn";
        let rustls_filter = "rustls=warn";

        let env_filter = EnvFilter::new(format!(
            "{},{},{},{},{}",
            default_filter, sqlx_filter, hyper_filter, h2_filter, rustls_filter
        ));

        tracing_subscriber::fmt().with_env_filter(env_filter).init();

        // Configure Bitcoin chain source from config
        let chain_source = ChainSource::BitcoinRpc(BitcoinRpcConfig {
            host: config.bitcoin.rpc_host.clone(),
            port: config.bitcoin.rpc_port,
            user: config.bitcoin.rpc_user.clone(),
            password: config.bitcoin.rpc_password.clone(),
        });

        // Configure LDK node
        let ldk_node_listen_addr = SocketAddress::from_str(&format!(
            "{}:{}",
            config.ldk.listen_host, config.ldk.listen_port
        ))
        .unwrap();

        let localstore = Arc::new(cdk_redb::WalletRedbDatabase::new(
            &work_dir.join("cdk-wallet.redb"),
        )?);

        let seed = Mnemonic::generate(12)?;

        let mut wallets = vec![];

        for mint in config.lsp.accepted_mints.iter() {
            let wallet = Wallet::new(
                mint,
                CurrencyUnit::Sat,
                localstore.clone(),
                &seed.to_seed_normalized(""),
                None,
            )?;
            wallets.push(wallet);
        }

        let wallet = MultiMintWallet::new(wallets);

        let cdk_ldk = cdk_ldk_node::CashuLspNode::new(
            chain_source,
            GossipSource::P2P,
            vec![ldk_node_listen_addr],
            wallet,
        )?;

        cdk_ldk.start(Some(runtime_clone))?;

        let cdk_ldk = Arc::new(cdk_ldk);

        let fund_addr = cdk_ldk.inner.onchain_payment().new_address()?;

        tracing::info!("Funding addr: {}", fund_addr);

        // Start gRPC management server
        let grpc_addr =
            format!("{}:{}", config.grpc.host, config.grpc.port).parse::<SocketAddr>()?;
        let management_service = CdkLdkServer::new(cdk_ldk.clone());

        let grpc_server = Server::builder()
            .add_service(CdkLdkManagementServer::new(management_service))
            .serve(grpc_addr);

        tokio::spawn(grpc_server);

        // Configure LSP server
        let cashu_lsp_info = CashuLspInfo {
            min_channel_size_sat: config.lsp.min_channel_size_sat,
            max_channel_size_sat: config.lsp.max_channel_size_sat,
            accepted_mints: config
                .lsp
                .accepted_mints
                .clone()
                .iter()
                .map(|s| MintUrl::from_str(s))
                .collect::<Result<Vec<MintUrl>, _>>()?,
            min_fee: config.lsp.min_fee,
            fee_ppk: config.lsp.fee_ppk,
        };

        let payment_url = config.lsp.payment_url.clone();

        let db = Db::new(work_dir.join("cashu-lsp.redb"))?;

        let service =
            create_cashu_lsp_router(Arc::clone(&cdk_ldk), cashu_lsp_info, payment_url, db).await?;

        let service = service.layer(CorsLayer::permissive());

        // Start LSP HTTP server
        let socket_addr = SocketAddr::from_str(&format!(
            "{}:{}",
            config.lsp.listen_host, config.lsp.listen_port
        ))?;

        tracing::info!("Starting LSP server on {}", socket_addr);

        let listener = tokio::net::TcpListener::bind(socket_addr).await?;

        let axum_result = axum::serve(listener, service).with_graceful_shutdown(shutdown_signal());

        match axum_result.await {
            Ok(_) => {
                tracing::info!("Axum server stopped with okay status");
            }
            Err(err) => {
                tracing::warn!("Axum server stopped with error");
                tracing::error!("{}", err);
                bail!("Axum exited with error")
            }
        }

        // Wait for shutdown signal
        signal::ctrl_c().await?;

        cdk_ldk.stop()?;

        Ok(())
    })
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("Shutdown signal received");
}
