use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, extract::Json, extract::State};
use cdk::amount::{Amount, SplitTarget};
use cdk::mint_url::MintUrl;
use cdk::nuts::CurrencyUnit;
use cdk::nuts::{PaymentRequest, PaymentRequestPayload, Transport, TransportType};
use cdk::wallet::types::WalletKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::CashuLspNode;
use crate::db::Db;
use crate::types::{ChannelQuoteRequest, QuoteInfo, QuoteState};

/// Cashu Lsp State
#[derive(Clone)]
pub struct CashuLspState {
    node: Arc<CashuLspNode>,
    cashu_lsp_info: CashuLspInfo,
    payment_url: String,
    db: Db,
}

pub async fn create_cashu_lsp_router(
    node: Arc<CashuLspNode>,
    lsp_info: CashuLspInfo,
    payment_url: String,
    db: Db,
) -> anyhow::Result<Router> {
    let state = CashuLspState {
        node,
        cashu_lsp_info: lsp_info,
        payment_url,
        db,
    };

    let router = Router::new()
        .route("/info", get(get_lsp_info))
        .route("/channel-quote", post(post_channel_quote))
        .route("/payment", post(post_receive_payment))
        .route("/quote/{id}", get(get_quote_state))
        .with_state(state);

    Ok(router)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashuLspInfo {
    pub min_channel_size_sat: u64,
    pub max_channel_size_sat: u64,
    pub accepted_mints: Vec<MintUrl>,
    pub min_fee: u64,
    pub fee_ppk: u64,
}

#[derive(Debug)]
pub enum LspError {
    InvalidUuid(String),
    QuoteNotFound(Uuid),
    InvalidChannelSize { size: u64, min: u64, max: u64 },
    UnsupportedMint(MintUrl),
    InvalidQuoteState { id: Uuid, state: QuoteState },
    InsufficientPayment { expected: u64, received: u64 },
    DatabaseError(String),
    ChannelOpenError(String),
    WalletError(String),
    ProofVerificationError(String),
    InternalError(String),
}

impl fmt::Display for LspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUuid(id) => write!(f, "Invalid UUID format: {}", id),
            Self::QuoteNotFound(id) => write!(f, "Quote not found: {}", id),
            Self::InvalidChannelSize { size, min, max } => {
                write!(
                    f,
                    "Channel size {} outside allowed range ({}-{})",
                    size, min, max
                )
            }
            Self::UnsupportedMint(mint) => write!(f, "Unsupported mint: {}", mint),
            Self::InvalidQuoteState { id, state } => {
                write!(f, "Quote {} has invalid state: {:?}", id, state)
            }
            Self::InsufficientPayment { expected, received } => {
                write!(
                    f,
                    "Insufficient payment: expected {}, received {}",
                    expected, received
                )
            }
            Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            Self::ChannelOpenError(msg) => write!(f, "Failed to open channel: {}", msg),
            Self::WalletError(msg) => write!(f, "Wallet error: {}", msg),
            Self::ProofVerificationError(msg) => write!(f, "Proof verification error: {}", msg),
            Self::InternalError(msg) => write!(f, "Internal server error: {}", msg),
        }
    }
}

impl IntoResponse for LspError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::InvalidUuid(_)
            | Self::InvalidChannelSize { .. }
            | Self::UnsupportedMint(_)
            | Self::InvalidQuoteState { .. }
            | Self::InsufficientPayment { .. } => StatusCode::BAD_REQUEST,

            Self::QuoteNotFound(_) => StatusCode::NOT_FOUND,

            Self::DatabaseError(_)
            | Self::ChannelOpenError(_)
            | Self::WalletError(_)
            | Self::ProofVerificationError(_)
            | Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        tracing::error!("LSP error: {}", self);
        (status, self.to_string()).into_response()
    }
}

pub async fn get_lsp_info(
    State(state): State<CashuLspState>,
) -> Result<Json<CashuLspInfo>, Response> {
    tracing::debug!("Handling LSP info request");
    Ok(Json(state.cashu_lsp_info))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelQuoteResponse {
    payment_request: String,
}

pub async fn post_channel_quote(
    State(state): State<CashuLspState>,
    Json(payload): Json<ChannelQuoteRequest>,
) -> Result<Json<ChannelQuoteResponse>, LspError> {
    tracing::debug!("Received channel quote request: {:?}", payload);

    // Validate channel size
    if payload.channel_size_sats > state.cashu_lsp_info.max_channel_size_sat {
        return Err(LspError::InvalidChannelSize {
            size: payload.channel_size_sats,
            min: state.cashu_lsp_info.min_channel_size_sat,
            max: state.cashu_lsp_info.max_channel_size_sat,
        });
    }

    if payload.channel_size_sats < state.cashu_lsp_info.min_channel_size_sat {
        return Err(LspError::InvalidChannelSize {
            size: payload.channel_size_sats,
            min: state.cashu_lsp_info.min_channel_size_sat,
            max: state.cashu_lsp_info.max_channel_size_sat,
        });
    }

    let fee = payload
        .channel_size_sats
        .checked_div(1_000)
        .expect("Amount overflow")
        .checked_mul(state.cashu_lsp_info.fee_ppk)
        .expect("Amount overflow");

    let fee = if fee < state.cashu_lsp_info.min_fee {
        state.cashu_lsp_info.min_fee
    } else {
        fee
    };

    let payment_id = Uuid::new_v4();

    let transport = Transport::builder()
        .transport_type(TransportType::HttpPost)
        .target(state.payment_url)
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build transport: {}", e);
            LspError::InternalError(format!("Failed to build transport: {}", e))
        })?;

    let payment_required = payload
        .channel_size_sats
        .checked_add(fee)
        .expect("amount overflow")
        .checked_add(payload.push_amount.unwrap_or_default())
        .expect("amount overflow");

    let payment_request = PaymentRequest::builder()
        .payment_id(payment_id)
        .amount(payment_required)
        .unit(CurrencyUnit::Sat)
        .single_use(true)
        .mints(state.cashu_lsp_info.accepted_mints)
        .add_transport(transport)
        .build();

    let quote = QuoteInfo {
        id: payment_id,
        channel_size_sats: payload.channel_size_sats,
        push_amount_sats: payload.push_amount,
        expected_payment_sats: payment_required,
        node_pubkey: payload.node_pubkey,
        addr: payload.addr,
        state: QuoteState::Unpaid,
        channel_id: None,
    };

    state.db.add_quote(&quote).map_err(|e| {
        tracing::error!("Failed to add quote to database: {}", e);
        LspError::DatabaseError(e.to_string())
    })?;

    tracing::info!("Created new channel quote: {}", payment_id);

    Ok(Json(ChannelQuoteResponse {
        payment_request: payment_request.to_string(),
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteStateResponse {
    pub id: Uuid,
    pub state: QuoteState,
    pub channel_id: Option<String>,
}

pub async fn get_quote_state(
    State(state): State<CashuLspState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<QuoteStateResponse>, LspError> {
    tracing::debug!("Received quote state request for ID: {}", id);

    let id = Uuid::from_str(&id).map_err(|e| {
        tracing::warn!("Invalid UUID format: {} - {}", id, e);
        LspError::InvalidUuid(id.clone())
    })?;

    let quote = state.db.get_quote(id).map_err(|e| {
        tracing::warn!("Quote not found: {} - {}", id, e);
        LspError::QuoteNotFound(id)
    })?;

    let mut channel_id = None;

    if let Some(user_channel_id) = quote.channel_id {
        let all_channel = state.node.inner.list_channels();

        let channel: Vec<&ldk_node::ChannelDetails> = all_channel
            .iter()
            .filter(|c| c.user_channel_id == user_channel_id)
            .collect();

        if let Some(channel_info) = channel.get(0) {
            channel_id = Some(channel_info.channel_id.to_string());
        } else {
            tracing::info!("Unkown channel for Channel user id: {}", user_channel_id.0);
        }
    }

    let response = QuoteStateResponse {
        id: quote.id,
        state: quote.state,
        channel_id,
    };

    tracing::debug!("Returning quote state for {}: {:?}", id, response);
    Ok(Json(response))
}

pub async fn post_receive_payment(
    State(state): State<CashuLspState>,
    Json(payload): Json<PaymentRequestPayload>,
) -> Result<(), LspError> {
    tracing::debug!("Received payment for mint: {}", payload.mint);

    // Validate mint
    if !state.cashu_lsp_info.accepted_mints.contains(&payload.mint) {
        return Err(LspError::UnsupportedMint(payload.mint.clone()));
    }

    // Validate payment ID
    let id = payload.id.ok_or_else(|| {
        tracing::warn!("Missing payment ID in request");
        LspError::InvalidUuid("missing".to_string())
    })?;

    let id = Uuid::from_str(&id).map_err(|e| {
        tracing::warn!("Invalid UUID format: {} - {}", id, e);
        LspError::InvalidUuid(id.clone())
    })?;

    // Get quote
    let quote = state.db.get_quote(id).map_err(|e| {
        tracing::warn!("Quote not found: {} - {}", id, e);
        LspError::QuoteNotFound(id)
    })?;

    // Validate quote state
    if quote.state != QuoteState::Unpaid {
        tracing::warn!("Quote {} has invalid state: {:?}", id, quote.state);
        return Err(LspError::InvalidQuoteState {
            id,
            state: quote.state,
        });
    }

    // Validate payment amount
    let received_amount =
        Amount::try_sum(payload.proofs.iter().map(|p| p.amount)).map_err(|e| {
            tracing::warn!("Failed to sum proof amounts: {}", e);
            LspError::InternalError("Failed to sum proof amounts".to_string())
        })?;

    if Amount::from(quote.expected_payment_sats) < received_amount {
        tracing::warn!(
            "Insufficient payment: expected {}, received {}",
            quote.expected_payment_sats,
            received_amount
        );
        return Err(LspError::InsufficientPayment {
            expected: quote.expected_payment_sats,
            received: received_amount.into(),
        });
    }

    // Get wallet for the mint
    let wallet = state
        .node
        .wallet
        .get_wallet(&WalletKey::new(payload.mint.clone(), CurrencyUnit::Sat))
        .await
        .ok_or_else(|| {
            let msg = format!("Wallet not created for {}", payload.mint);
            tracing::warn!("{}", msg);
            LspError::WalletError(msg)
        })?;

    // Receive and verify proofs
    let amount = wallet
        .receive_proofs(payload.proofs, SplitTarget::default(), &[], &[])
        .await
        .map_err(|e| {
            tracing::error!("Could not receive proofs for {}: {}", id, e);
            LspError::ProofVerificationError(e.to_string())
        })?;

    tracing::info!(
        "Successfully received payment of {} sats for quote {}",
        amount,
        id
    );

    // Update quote state
    let mut quote = state
        .db
        .update_quote_state(id, QuoteState::ChannelPending)
        .map_err(|e| {
            tracing::error!("Failed to update quote state: {}", e);
            LspError::DatabaseError(e.to_string())
        })?;

    // Try to open the channel
    tracing::info!(
        "Opening channel to {} with {} sats (push: {:?})",
        quote.node_pubkey,
        quote.channel_size_sats,
        quote.push_amount_sats
    );

    let open_channel = state.node.inner.open_announced_channel(
        quote.node_pubkey,
        quote.addr.clone(),
        quote.channel_size_sats,
        quote.push_amount_sats.map(|a| a * 1_000),
        None,
    );

    match open_channel {
        Ok(channel_id) => {
            tracing::info!("Successfully opened channel with ID: {}", channel_id.0);
            quote.channel_id = Some(channel_id);
            quote.state = QuoteState::ChannelOpen;
            state.db.add_quote(&quote).map_err(|e| {
                tracing::error!("Failed to update quote with channel info: {}", e);
                LspError::DatabaseError(e.to_string())
            })?;
        }
        Err(err) => {
            tracing::error!("Could not open channel for quote {}: {}", quote.id, err);
            quote.state = QuoteState::Paid;
            state.db.add_quote(&quote).map_err(|e| {
                tracing::error!(
                    "Failed to update quote state after channel open failure: {}",
                    e
                );
                LspError::DatabaseError(e.to_string())
            })?;
        }
    }

    tracing::info!("Payment processing completed for quote {}", id);
    Ok(())
}
