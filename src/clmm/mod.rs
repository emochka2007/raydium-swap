pub mod clmm_instructions;

pub use clmm_instructions::*;
use reqwest::Client;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_response::transaction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;

pub mod clmm_utils;
pub use clmm_utils::*;
pub mod clmm_math;
pub use clmm_math::*;
pub mod process_clmm_commands;
pub use process_clmm_commands::*;
pub mod clmm_types;
pub use clmm_types::*;
pub mod decode_clmm_ix_event;
pub use decode_clmm_ix_event::*;

pub struct ClmmConfig {
    clmm_program: Option<Pubkey>,
}

pub struct ClmmSwapClient {
    reqwest_client: Client,
    base_url: String,
    owner: Keypair,
    rpc_client: RpcClient,
}

impl ClmmSwapClient {
    pub fn new(rpc_client: RpcClient, owner: Keypair) -> Self {
        let reqwest_client = Client::new();
        let base_url = "https://api-v3.raydium.io".to_string();
        Self {
            rpc_client,
            base_url,
            owner,
            reqwest_client,
        }
    }
}
