use raydium_amm_swap::amm::client::AmmSwapClient;
use raydium_amm_swap::consts::SOL_MINT;
use raydium_amm_swap::interface::{PoolSortField, PoolType};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use std::env;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    dotenvy::dotenv().ok();

    let rpc_url = env::var("RPC_URL").unwrap();
    let mint_a = env::var("MINT_1").unwrap_or_else(|_| SOL_MINT.to_string());
    let mint_b = env::var("MINT_2").expect("Set MINT_2 to the quote token mint");

    // Fetch concentrated pools sorted by the chosen field.
    let sort_field = PoolSortField::Volume24h;
    let sort_field_param = sort_field.to_string();

    let client = AmmSwapClient::new(RpcClient::new(rpc_url), Keypair::new());
    let pools = client
        .fetch_pool_info(
            &mint_a,
            &mint_b,
            &PoolType::Concentrated,
            Some(5),
            Some(1),
            Some(sort_field_param.as_str()),
            Some("desc"),
        )
        .await
        .expect("failed to fetch pools");

    let ids: Vec<&str> = pools.iter().map(|pool| pool.id.as_str()).collect();
    info!("Top pools sorted by {}: {:?}", sort_field, ids);
}
