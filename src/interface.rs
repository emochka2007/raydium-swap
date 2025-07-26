use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PoolInfosResponse {
    pub(crate) id: String,
    success: bool,
    pub(crate) data: PoolInfosResponseData
}

#[derive(Deserialize, Debug)]
pub struct PoolInfosResponseData {
    count: Option<u32>,
    pub(crate) data: Vec<PoolInfoData>
}

#[derive(Deserialize, Debug)]
pub struct PoolInfoResponse {
    pub(crate) data: Vec<PoolInfoData>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfoData {
    r#type: String,
    program_id: String,
    id: String,
    pub(crate) mint_a: Mint,
    pub(crate) mint_b: Mint,
    price: f64,
    mint_amount_a: f64,
    mint_amount_b: f64,
    fee_rate: f64,
    open_time: String,
    tvl: f64,
    day: PoolInfoPeriodData,
    week: PoolInfoPeriodData,
    month: PoolInfoPeriodData,
    pool_type: Option<Vec<String>>,
    reward_default_pool_infos: Option<String>,
    reward_default_infos: Vec<RewardDefaultInfo>,
    farm_upcoming_count: u32,
    farm_ongoing_count: u32,
    farm_finished_count: u32,
    market_id: Option<String>,
    lp_mint: Mint,
    lp_price: f64,
    lp_amount: f64,
    burn_percent: f64,
    launch_migrate_pool: bool
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfoPeriodData {
    volume: f64,
    volume_quote: f64,
    volume_fee: f64,
    apr: f64,
    fee_apr: f64,
    price_min: f64,
    price_max: f64,
    reward_apr: Vec<u32>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RewardDefaultInfo {
    mint: Mint,
    per_second: String,
    start_time: String,
    end_time: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Mint {
    chain_id: u32,
    pub(crate) address: String,
    program_id: String,
    logo_uri: Option<String>,
    symbol: String,
    name: String,
    pub(crate) decimals: u32,
    tags: Vec<String>,
    extensions: MintExtensions,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MintExtensions {}


#[derive(Deserialize, Debug)]
pub struct PoolKeysResponse {
    id: String,
    success: bool,
    pub(crate) data: Vec<PoolKey>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PoolKey {
    program_id: String,
    pub(crate) id: String,
    pub(crate) mint_a: Mint,
    pub(crate) mint_b: Mint,
    lookup_table_account: String,
    open_time: String,
    pub(crate) vault: Vault,
    pub(crate) authority: String,
    pub(crate) open_orders: String,
    target_orders: String,
    mint_lp: Mint,
    pub(crate) market_program_id: String,
    pub(crate) market_id: String,
    pub(crate) market_authority: String,
    pub(crate) market_base_vault: String,
    pub(crate) market_quote_vault: String,
    pub(crate) market_bids: String,
    pub(crate) market_asks: String,
    pub(crate) market_event_queue: String
}

#[derive(Deserialize, Debug)]
pub struct Vault {
    #[serde(rename="A")]
    pub(crate) a: String,
    #[serde(rename="B")]
    pub(crate) b: String
}