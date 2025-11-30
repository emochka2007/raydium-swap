//! Types for deserializing JSON responses from the Raydium HTTP API.

use serde::Deserialize;
use solana_address::Address;
use solana_sdk::pubkey::Pubkey;
use std::fmt::Display;

/// Response from `/pools/info/mint`.
#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct PoolInfosResponse {
    /// The request ID.
    pub id: String,
    /// Whether the API call was successful.
    pub success: bool,
    /// The payload data.
    pub data: ManyPoolsInfo,
}

/// Combined response type for standard vs. concentrated pools.
///
/// This is returned by `AmmSwapClient::fetch_pool_info` and allows callers to
/// match on the concrete shape at compile time based on `PoolType`.
#[derive(Debug)]
pub enum PoolInfosByType {
    /// Standard AMM v4 pool response.
    Standard(PoolInfosResponse),
    /// Concentrated (CLMM) pool response.
    Concentrated(ClmmPoolInfosResponse),
}

/// Metadata and list of pools.
#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct ManyPoolsInfo {
    pub count: Option<u32>,
    pub data: Vec<Pool>,
}

/// Response from `/pools/info/ids`.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct SinglePoolInfo {
    pub data: Vec<Pool>,
}

/// Response from `/pools/info/ids` for concentrated (CLMM) pools.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ClmmSinglePoolInfo {
    pub data: Vec<ClmmPool>,
}

/// Combined response type for single‑pool lookups by ID.
#[derive(Debug)]
pub enum SinglePoolInfoByType {
    /// Standard AMM v4 pool response.
    Standard(SinglePoolInfo),
    /// Concentrated (CLMM) pool response.
    Concentrated(ClmmSinglePoolInfo),
}

/// Detailed information for a single pool.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pool {
    /// Type of pool (e.g., “standard”).
    pub r#type: String,
    /// On‑chain program ID.
    pub program_id: String,
    /// Pool account address.
    pub id: String,
    /// Token A mint information.
    pub mint_a: Mint,
    /// Token B mint information.
    pub mint_b: Mint,
    /// Current pool price (token B per token A).
    pub price: f64,
    /// Token A reserve amount.
    pub mint_amount_a: f64,
    /// Token B reserve amount.
    pub mint_amount_b: f64,
    /// Fee rate applied on swaps.
    pub fee_rate: f64,
    /// Pool creation timestamp.
    pub open_time: String,
    /// Total value locked.
    pub tvl: f64,
    /// 24‑hour stats.
    pub day: PoolPeriod,
    /// 7‑day stats.
    pub week: PoolPeriod,
    /// 30‑day stats.
    pub month: PoolPeriod,
    /// Optional pool subtype tags.
    pub pool_type: Option<Vec<String>>,
    /// Default rewards info.
    pub reward_default_pool_infos: Option<String>,
    /// List of per‑reward distributions.
    pub reward_default_infos: Vec<RewardDefault>,
    /// Counts of associated farms.
    pub farm_upcoming_count: u32,
    pub farm_ongoing_count: u32,
    pub farm_finished_count: u32,
    /// On‑chain market ID for concentrated liquidity.
    pub market_id: Option<String>,
    /// LP token mint.
    pub lp_mint: Mint,
    /// Price of LP token.
    pub lp_price: f64,
    /// Amount of LP tokens in circulation.
    pub lp_amount: f64,
    /// Percent of LP tokens burned.
    pub burn_percent: f64,
    /// Whether migration is required.
    pub launch_migrate_pool: bool,
}

/// Period‑specific stats for a pool.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PoolPeriod {
    /// Trading volume in base token.
    pub volume: f64,
    /// Trading volume in quote currency.
    pub volume_quote: f64,
    /// Fees collected.
    pub volume_fee: f64,
    /// Annualized percentage rate.
    pub apr: f64,
    /// Fee APR.
    pub fee_apr: f64,
    /// Minimum price observed.
    pub price_min: f64,
    /// Maximum price observed.
    pub price_max: f64,
    /// Reward APRs (per reward mint).
    pub reward_apr: Vec<u32>,
}

/// Info about a default reward stream.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RewardDefault {
    pub mint: Mint,
    pub per_second: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

/// Token mint metadata.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Mint {
    pub chain_id: u32,
    pub address: String,
    pub program_id: String,
    pub logo_uri: Option<String>,
    pub symbol: String,
    pub name: String,
    pub decimals: u32,
    pub tags: Vec<String>,
    pub extensions: MintExtensions,
}

/// Empty placeholder for mint extensions.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MintExtensions {}

/// Response from `/pools/key/ids`.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct PoolKeys {
    pub id: String,
    pub success: bool,
    pub data: Vec<SinglePoolKey>,
}

/// On‑chain account addresses needed for swaps.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SinglePoolKey {
    /// AMM program ID.
    pub program_id: String,
    /// Pool account address.
    pub id: String,
    pub mint_a: Mint,
    pub mint_b: Mint,
    pub lookup_table_account: String,
    pub open_time: String,
    pub vault: Vault,
    pub authority: String,
    pub open_orders: String,
    pub target_orders: String,
    pub market_program_id: String,
    pub market_id: String,
    pub market_authority: String,
    pub market_base_vault: String,
    pub market_quote_vault: String,
    pub market_bids: String,
    pub market_asks: String,
    pub market_event_queue: String,
}

/// Vault addresses for token A and B.
#[derive(Deserialize, Debug)]
pub struct Vault {
    #[serde(rename = "A")]
    pub a: String,
    #[serde(rename = "B")]
    pub b: String,
}

pub enum PoolType {
    Standard,
    Concentrated,
}

impl Display for PoolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolType::Standard => write!(f, "{}", "standard"),
            PoolType::Concentrated => write!(f, "{}", "concentrated"),
        }
    }
}

/// Response for concentrated (CLMM) pools, e.g.:
/// `/clmm/pools/info/mint` or `/pools/info/mint` with `poolType=concentrated`.
#[derive(Deserialize, Debug)]
pub struct ClmmPoolInfosResponse {
    /// The request ID.
    pub id: String,
    /// Whether the API call was successful.
    pub success: bool,
    /// The payload data.
    pub data: ClmmManyPoolsInfo,
}

/// Metadata and list of concentrated pools.
#[derive(Deserialize, Debug)]
pub struct ClmmManyPoolsInfo {
    pub count: Option<u32>,
    pub data: Vec<ClmmPool>,
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
}

/// CLMM‑specific pool config block.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClmmConfig {
    pub id: String,
    pub index: u32,
    pub protocol_fee_rate: u64,
    pub trade_fee_rate: u64,
    pub tick_spacing: u64,
    pub fund_fee_rate: u64,
    pub default_range: f64,
    pub default_range_point: Vec<f64>,
}

/// Detailed information for a concentrated (CLMM) pool.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClmmPool {
    /// Type of pool, e.g. "Concentrated".
    pub r#type: String,
    /// On‑chain program ID.
    pub program_id: String,
    /// Pool account address.
    pub id: String,
    /// Token A mint information.
    pub mint_a: Mint,
    /// Token B mint information.
    pub mint_b: Mint,
    /// Default rewards info, e.g. "Clmm".
    pub reward_default_pool_infos: Option<String>,
    /// List of per‑reward distributions.
    pub reward_default_infos: Vec<RewardDefault>,
    /// Current pool price (token B per token A).
    pub price: f64,
    /// Token A reserve amount.
    pub mint_amount_a: f64,
    /// Token B reserve amount.
    pub mint_amount_b: f64,
    /// Fee rate applied on swaps.
    pub fee_rate: f64,
    /// Pool creation timestamp.
    pub open_time: String,
    /// Total value locked.
    pub tvl: f64,
    /// 24‑hour stats.
    pub day: PoolPeriod,
    /// 7‑day stats.
    pub week: PoolPeriod,
    /// 30‑day stats.
    pub month: PoolPeriod,
    /// Pool subtype tags (note: JSON key is `pooltype`).
    #[serde(rename = "pooltype")]
    pub pool_type: Option<Vec<String>>,
    /// Counts of associated farms.
    pub farm_upcoming_count: u32,
    pub farm_ongoing_count: u32,
    pub farm_finished_count: u32,
    /// CLMM config (ticks, fees, ranges).
    pub config: ClmmConfig,
    /// Percent of LP tokens burned.
    pub burn_percent: f64,
    /// Whether migration is required.
    pub launch_migrate_pool: bool,
}

pub struct ClmmSwapParams {
    pub pool_id: solana_pubkey::Pubkey,
    /// The token of user want to swap from.
    pub user_input_token: solana_pubkey::Pubkey,
    /// The token of user want to swap to.
    /// If none is given, the account will be ATA account.
    pub user_output_token: Option<solana_pubkey::Pubkey>,
    /// The amount specified of user want to swap from or to token.
    pub amount_specified: u64,
    /// The float price of the pool that can be swaped to.
    pub limit_price: Option<f64>,
    /// The amount specified is output_token or not.
    pub base_out: bool,
    /// Slippage for the swap in bps
    pub slippage_bps: u64,
}
