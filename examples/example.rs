use anyhow::anyhow;
use raydium_amm_swap::helpers::from_bytes_to_key_pair;
use raydium_amm_swap::interface::{PoolInfosByType, PoolType, SinglePoolInfoByType};
use raydium_amm_swap::raydium::AmmSwapClient;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::env;
use std::str::FromStr;
use tracing::info;

// todo Add CLMM support
#[tokio::main]
async fn main() {
    dotenvy::dotenv().unwrap();
    let amount_in = 500;
    let slippage = 0.01;
    let url = env::var("RPC_URL").unwrap();
    let mint_a = env::var("MINT_1").unwrap();
    let mint_b = env::var("MINT_2").unwrap();
    let rpc_client = RpcClient::new(url);
    let owner = env::var("KEYPAIR").expect("KEYPAIR env is not presented");
    let amm_swap_client = AmmSwapClient::new(rpc_client, from_bytes_to_key_pair(owner));

    // Choose which kind of pool to query.
    let pool_type = PoolType::Concentrated;

    let all_mint_pools = amm_swap_client
        .fetch_pool_info(&mint_a, &mint_b, &pool_type, None, None, None, None)
        .await
        .unwrap();

    println!("{:?}", all_mint_pools);

    let pool_id_str = match &all_mint_pools {
        PoolInfosByType::Standard(pools) => &pools.data.data.first().unwrap().id,
        PoolInfosByType::Concentrated(pools) => &pools.data.data.first().unwrap().id,
    };

    let pool_id = Pubkey::from_str(pool_id_str).unwrap();

    let pool_info = amm_swap_client
        .fetch_pool_by_id(&pool_id, &pool_type)
        .await
        .unwrap();

    let pool_keys = amm_swap_client
        .fetch_pools_keys_by_id(&pool_id)
        .await
        .unwrap();

    let rpc_data = amm_swap_client
        .get_rpc_pool_info(&pool_id)
        .await
        .map_err(|e| anyhow!("Error fetching rpc pool info {e:?}"))
        .unwrap();

    // For now, compute_amount_out & swap are only wired for standard AMM v4.
    if let (PoolType::Standard, SinglePoolInfoByType::Standard(info)) = (pool_type, &pool_info) {
        let pool = info.data.get(0).unwrap();
        let compute = amm_swap_client
            .compute_amount_out(&rpc_data, pool, amount_in, slippage)
            .unwrap();

        let key = pool_keys.data.get(0).unwrap();
        info!("Standard pool key: {:?}", key);

        // let _sig = amm_swap_client
        //     .swap(key, &mint_a, &mint_b, amount_in, compute.amount_out)
        //     .await
        //     .unwrap();
        let _ = compute; // silence unused warning when swap is commented
    } else {
        info!(
            "CLMM single‑pool info fetched successfully; CLMM math not yet implemented in this example."
        );
    }
    assert!(true);
}
