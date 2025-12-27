//! Types for deserializing JSON responses from the Raydium HTTP API.

use crate::states::{AmmConfig, TickArrayBitmapExtension, TickArrayState};
use serde::Deserialize;
use serde_json::Value;
use solana_account::Account;
use solana_address::Address;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};

/// Response from `/pools/info/ids` for concentrated (CLMM) pools.
#[derive(Deserialize, Debug)]
pub struct ClmmSinglePoolInfo {
    pub data: Vec<ClmmPool>,
}

/// Period‑specific stats for a pool.
#[derive(Deserialize, Debug, Clone)]
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
    pub reward_apr: Vec<f64>,
}

/// Info about a default reward stream.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RewardDefault {
    pub mint: Mint,
    pub per_second: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

/// Token mint metadata.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
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
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MintExtensions {}

/// Response from `/pools/key/ids`.
#[derive(Deserialize, Debug, Clone)]
pub struct PoolKeys<PoolType: Clone> {
    pub id: String,
    pub success: bool,
    pub data: Vec<PoolType>,
}

/// On‑chain account addresses needed for swaps.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AmmPool {
    /// AMM program ID.
    pub program_id: String,
    /// Pool account address.
    pub id: String,
    pub mint_a: Mint,
    pub mint_b: Mint,
    pub lookup_table_account: Option<String>,
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
#[derive(Deserialize, Debug, Clone)]
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
            PoolType::Standard => write!(f, "standard"),
            PoolType::Concentrated => write!(f, "concentrated"),
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
    pub data: Vec<Value>,
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
}

/// CLMM‑specific pool config block.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[derive(Clone)]
pub struct ClmmConfig {
    pub id: String,
    pub index: u32,
    pub protocol_fee_rate: u64,
    pub trade_fee_rate: u64,
    pub tick_spacing: Option<u64>,
    pub fund_fee_rate: Option<u64>,
    pub default_range: Option<f64>,
    pub default_range_point: Option<Vec<f64>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClmmPool {
    /// Type of pool, e.g. "Concentrated".
    pub r#type: Option<String>,
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
    pub reward_default_infos: Option<Vec<RewardDefault>>,
    /// Current pool price (token B per token A).
    pub price: Option<f64>,
    /// Token A reserve amount.
    pub mint_amount_a: Option<f64>,
    /// Token B reserve amount.
    pub mint_amount_b: Option<f64>,
    /// Fee rate applied on swaps.
    pub fee_rate: Option<f64>,
    /// Pool creation timestamp.
    pub open_time: Option<String>,
    /// Total value locked.
    pub tvl: Option<f64>,
    /// 24‑hour stats.
    pub day: Option<PoolPeriod>,
    /// 7‑day stats.
    pub week: Option<PoolPeriod>,
    /// 30‑day stats.
    pub month: Option<PoolPeriod>,
    /// Pool subtype tags (note: JSON key is `pooltype`).
    #[serde(rename = "pooltype")]
    pub pool_type: Option<Vec<String>>,
    /// Counts of associated farms.
    pub farm_upcoming_count: Option<u32>,
    pub farm_ongoing_count: Option<u32>,
    pub farm_finished_count: Option<u32>,
    /// CLMM config (ticks, fees, ranges).
    pub config: Option<ClmmConfig>,
    /// Percent of LP tokens burned.
    pub burn_percent: Option<f64>,
    /// Whether migration is required.
    pub launch_migrate_pool: Option<bool>,
}

#[cfg_attr(feature = "derive", derive(Debug))]
pub struct ClmmSwapParams {
    pub pool_id: solana_pubkey::Pubkey,
    /// The token of user want to swap from.
    pub user_input_token: solana_pubkey::Pubkey,
    /// The token of user want to swap to.
    /// If none is given, the account will be ATA account.
    pub user_output_token: solana_pubkey::Pubkey,
    /// The amount specified of user want to swap from or to token.
    pub amount_specified: u64,
    /// The float price of the pool that can be swaped to.
    pub limit_price: Option<f64>,
    /// The amount specified is output_token or not.
    pub base_out: bool,
    /// Slippage for the swap in bps
    pub slippage_bps: u64,
}

pub type Rsps = Vec<Option<Account>>;
pub type TickArrays = VecDeque<TickArrayState>;

#[cfg_attr(feature = "derive", derive(Debug))]
pub struct CalculateSwapChangeParams {
    pub tickarray_bitmap_extension_account: Option<Account>,
    pub amm_config_account: Option<Account>,
    pub user_input_state: spl_token_2022::state::Account,
    pub mint0_account: Option<Account>,
    pub mint1_account: Option<Account>,
    pub mint0_token_program: Address,
    pub mint1_token_program: Address,
    pub tickarray_bitmap_extension_state: TickArrayBitmapExtension,
    pub zero_for_one: bool,
    pub amount_specified: u64,
    pub amm_config_state: AmmConfig,
    pub input_vault: solana_pubkey::Pubkey,
    pub output_vault: solana_pubkey::Pubkey,
    pub input_vault_mint: solana_pubkey::Pubkey,
    pub output_vault_mint: solana_pubkey::Pubkey,
    pub input_token_program: Address,
    pub output_token_program: Address,
}

pub enum PoolSortField {
    Liquidity,
    Volume24h,
    Volume7d,
    Volume30d,
    Fee24h,
    Fee7d,
    Fee30d,
    Apr24h,
    Apr7d,
    Apr30d,
}

impl Display for PoolSortField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let sort = match self {
            Self::Liquidity => "liquidity",
            Self::Volume24h => "volume24h",
            Self::Volume7d => "volume_7d",
            Self::Volume30d => "volume_30d",
            Self::Fee24h => "fee_24h",
            Self::Fee7d => "fee_7d",
            Self::Fee30d => "fee_30d",
            Self::Apr24h => "apr_24h",
            Self::Apr7d => "apr_7d",
            Self::Apr30d => "apr_30d",
        };

        write!(f, "{}", sort)
    }
}

#[cfg(test)]
mod tests {
    use super::PoolSortField;

    #[test]
    fn pool_sort_field_display_outputs_expected_strings() {
        let cases = [
            (PoolSortField::Liquidity, "liquidity"),
            (PoolSortField::Volume24h, "volume_24h"),
            (PoolSortField::Volume7d, "volume7d"),
            (PoolSortField::Volume30d, "volume30d"),
            (PoolSortField::Fee24h, "fee24h"),
            (PoolSortField::Fee7d, "fee7d"),
            (PoolSortField::Fee30d, "fee30d"),
            (PoolSortField::Apr24h, "apr24h"),
            (PoolSortField::Apr7d, "apr7d"),
            (PoolSortField::Apr30d, "apr30d"),
        ];

        for (field, expected) in cases.iter() {
            assert_eq!(field.to_string(), *expected);
        }
    }
}
