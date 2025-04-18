use std::str::FromStr;
use std::sync::Arc;

use ldk_node::UserChannelId;
use ldk_node::bitcoin::Address;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::lightning::ln::msgs::SocketAddress;
use tonic::{Request, Response, Status};

use super::cdk_ldk_management_server::CdkLdkManagement;
use super::*;
use crate::CashuLspNode;

pub struct CdkLdkServer {
    node: Arc<CashuLspNode>,
}

impl CdkLdkServer {
    pub fn new(node: Arc<CashuLspNode>) -> Self {
        Self { node }
    }
}

#[tonic::async_trait]
impl CdkLdkManagement for CdkLdkServer {
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> Result<Response<GetInfoResponse>, Status> {
        Ok(Response::new(GetInfoResponse {}))
    }

    async fn get_new_address(
        &self,
        _request: Request<GetNewAddressRequest>,
    ) -> Result<Response<GetNewAddressResponse>, Status> {
        let address = self
            .node
            .inner
            .onchain_payment()
            .new_address()
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetNewAddressResponse {
            address: address.to_string(),
        }))
    }

    async fn open_channel(
        &self,
        request: Request<OpenChannelRequest>,
    ) -> Result<Response<OpenChannelResponse>, Status> {
        let req = request.into_inner();

        let socket_addr = SocketAddress::from_str(&format!("{}:{}", req.address, req.port))
            .map_err(|e| Status::internal(e.to_string()))?;

        let channel = self
            .node
            .inner
            .open_announced_channel(
                PublicKey::from_str(&req.node_id).map_err(|e| Status::internal(e.to_string()))?,
                socket_addr,
                req.amount_msats,
                req.push_to_counter_party_msats,
                None,
            )
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(OpenChannelResponse {
            channel_id: channel.0.to_string(),
        }))
    }

    async fn close_channel(
        &self,
        request: Request<CloseChannelRequest>,
    ) -> Result<Response<CloseChannelResponse>, Status> {
        let req = request.into_inner();

        let node_pubkey = req
            .node_pubkey
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid node pubkey: {}", e)))?;

        let channel_id: u128 = req
            .channel_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid channel id: {}", e)))?;

        let channel_id = UserChannelId(channel_id);

        self.node
            .inner
            .close_channel(&channel_id, node_pubkey)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CloseChannelResponse {}))
    }

    async fn list_balance(
        &self,
        _request: Request<ListBalanceRequest>,
    ) -> Result<Response<ListBalanceResponse>, Status> {
        let node_balance = self.node.inner.list_balances();

        Ok(Response::new(ListBalanceResponse {
            total_onchain_balance_sats: node_balance.total_onchain_balance_sats,
            spendable_onchain_balance_sats: node_balance.spendable_onchain_balance_sats,
            total_lightning_balance_sats: node_balance.total_lightning_balance_sats,
        }))
    }

    async fn send_onchain(
        &self,
        request: Request<SendOnchainRequest>,
    ) -> Result<Response<SendOnchainResponse>, Status> {
        let req = request.into_inner();

        let address =
            Address::from_str(&req.address).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let txid = self
            .node
            .inner
            .onchain_payment()
            .send_to_address(address.assume_checked_ref(), req.amount_sat)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(SendOnchainResponse {
            txid: txid.to_string(),
        }))
    }
}
