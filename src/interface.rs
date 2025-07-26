//! Types for deserializing JSON responses from the Raydium HTTP API.

use serde::Deserialize;

/// Response from `/pools/info/mint`.
#[derive(Deserialize, Debug)]
pub struct PoolInfosResponse {
    /// The request ID.
    pub(crate) id: String,
    /// Whether the API call was successful.
    success: bool,
    /// The payload data.
    pub(crate) data: PoolInfosResponseData,
}

/// Metadata and list of pools.
#[derive(Deserialize, Debug)]
pub struct PoolInfosResponseData {
    count: Option<u32>,
    pub(crate) data: Vec<PoolInfoData>,
}

/// Response from `/pools/info/ids`.
#[derive(Deserialize, Debug)]
pub struct PoolInfoResponse {
    pub(crate) data: Vec<PoolInfoData>,
}

/// Detailed information for a single pool.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfoData {
    /// Type of pool (e.g., “standard”).
    pub r#type: String,
    /// On‑chain program ID.
    pub program_id: String,
    /// Pool account address.
    pub id: String,
    /// Token A mint information.
    pub(crate) mint_a: Mint,
    /// Token B mint information.
    pub(crate) mint_b: Mint,
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
    pub day: PoolInfoPeriodData,
    /// 7‑day stats.
    pub week: PoolInfoPeriodData,
    /// 30‑day stats.
    pub month: PoolInfoPeriodData,
    /// Optional pool subtype tags.
    pub pool_type: Option<Vec<String>>,
    /// Default rewards info.
    pub reward_default_pool_infos: Option<String>,
    /// List of per‑reward distributions.
    pub reward_default_infos: Vec<RewardDefaultInfo>,
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
pub struct PoolInfoPeriodData {
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
pub struct RewardDefaultInfo {
    pub mint: Mint,
    pub per_second: String,
    pub start_time: String,
    pub end_time: String,
}

/// Token mint metadata.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Mint {
    pub chain_id: u32,
    pub(crate) address: String,
    pub program_id: String,
    pub logo_uri: Option<String>,
    pub symbol: String,
    pub name: String,
    pub(crate) decimals: u32,
    pub tags: Vec<String>,
    pub extensions: MintExtensions,
}

/// Empty placeholder for mint extensions.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MintExtensions {}

/// Response from `/pools/key/ids`.
#[derive(Deserialize, Debug)]
pub struct PoolKeysResponse {
    pub id: String,
    success: bool,
    pub(crate) data: Vec<PoolKey>,
}

/// On‑chain account addresses needed for swaps.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PoolKey {
    /// AMM program ID.
    pub program_id: String,
    /// Pool account address.
    pub(crate) id: String,
    pub(crate) mint_a: Mint,
    pub(crate) mint_b: Mint,
    lookup_table_account: String,
    open_time: String,
    pub(crate) vault: Vault,
    pub(crate) authority: String,
    pub(crate) open_orders: String,
    target_orders: String,
    pub(crate) market_program_id: String,
    pub(crate) market_id: String,
    pub(crate) market_authority: String,
    pub(crate) market_base_vault: String,
    pub(crate) market_quote_vault: String,
    pub(crate) market_bids: String,
    pub(crate) market_asks: String,
    pub(crate) market_event_queue: String,
}

/// Vault addresses for token A and B.
#[derive(Deserialize, Debug)]
pub struct Vault {
    #[serde(rename="A")]
    pub(crate) a: String,
    #[serde(rename="B")]
    pub(crate) b: String,
}
