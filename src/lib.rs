use std::sync::Arc;

use cdk::wallet::MultiMintWallet;
use ldk_node::bitcoin::Network;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::{Builder, Node};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

pub mod config;
pub mod db;
pub mod lsp_server;
pub mod proto;
pub mod types;

pub use lsp_server::create_cashu_lsp_router;

pub struct CashuLspNode {
    pub inner: Arc<Node>,
    events_cancel_token: CancellationToken,
    wallet: MultiMintWallet,
}

#[derive(Debug, Clone)]
pub struct BitcoinRpcConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub enum ChainSource {
    Esplora(String),
    BitcoinRpc(BitcoinRpcConfig),
}

#[derive(Debug, Clone)]
pub enum GossipSource {
    P2P,
    RapidGossipSync(String),
}

impl CashuLspNode {
    pub fn new(
        chain_source: ChainSource,
        gossip_source: GossipSource,
        listening_address: Vec<SocketAddress>,
        wallet: MultiMintWallet,
    ) -> anyhow::Result<Self> {
        let builder = Builder::new();
        builder.set_network(Network::Regtest);

        match chain_source {
            ChainSource::Esplora(esplora_url) => {
                builder.set_chain_source_esplora(esplora_url, None);
            }
            ChainSource::BitcoinRpc(BitcoinRpcConfig {
                host,
                port,
                user,
                password,
            }) => {
                builder.set_chain_source_bitcoind_rpc(host, port, user, password);
            }
        }

        match gossip_source {
            GossipSource::P2P => {
                builder.set_gossip_source_p2p();
            }
            GossipSource::RapidGossipSync(rgs_url) => {
                builder.set_gossip_source_rgs(rgs_url);
            }
        }

        builder.set_listening_addresses(listening_address)?;

        builder.set_node_alias("Cdk-mint-node".to_string())?;

        let node = builder.build()?;

        Ok(Self {
            inner: node,
            events_cancel_token: CancellationToken::new(),
            wallet,
        })
    }

    pub fn start(&self, runtime: Option<Arc<Runtime>>) -> anyhow::Result<()> {
        match runtime {
            Some(runtime) => self.inner.start_with_runtime(runtime)?,
            None => self.inner.start()?,
        };
        tracing::info!("Started ldk node");

        Ok(())
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.events_cancel_token.cancel();
        self.inner.stop()?;
        Ok(())
    }
}
