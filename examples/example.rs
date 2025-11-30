use anyhow::anyhow;
use raydium_amm_swap::amm::client::AmmSwapClient;
use raydium_amm_swap::helpers::from_bytes_to_key_pair;
use raydium_amm_swap::interface::{
    AmmPool, ClmmPool, ClmmSwapParams, PoolInfosByType, PoolKeys, PoolType, SinglePoolInfo,
    SinglePoolInfoByType,
};
use solana_address::Address;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use spl_associated_token_account::get_associated_token_address;
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
    let keypair = from_bytes_to_key_pair(owner);
    let owner_pubkey = keypair.pubkey();
    println!("Owner address {}", owner_pubkey.to_string());
    let amm_swap_client = AmmSwapClient::new(rpc_client, keypair);

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

    // For now, compute_amount_out & swap are only wired for standard AMM v4.
    match (pool_type, pool_info) {
        (PoolType::Standard, SinglePoolInfoByType::Standard(info)) => {
            let pool_keys: PoolKeys<AmmPool> = amm_swap_client
                .fetch_pools_keys_by_id(&pool_id)
                .await
                .unwrap();

            let rpc_data = amm_swap_client
                .get_rpc_pool_info(&pool_id)
                .await
                .map_err(|e| anyhow!("Error fetching rpc pool info {e:?}"))
                .unwrap();
            let pool = info.data.get(0).unwrap();
            let compute = amm_swap_client
                .compute_amount_out(&rpc_data, pool, amount_in, slippage)
                .unwrap();

            let key = pool_keys.data.get(0).unwrap();
            info!("Standard pool key: {:?}", key);

            let _sig = amm_swap_client
                .swap(key, &mint_a, &mint_b, amount_in, compute.amount_out)
                .await
                .unwrap();
        }
        (PoolType::Concentrated, SinglePoolInfoByType::Concentrated(info)) => {
            let pool_keys: PoolKeys<ClmmPool> = amm_swap_client
                .fetch_pools_keys_by_id(&pool_id)
                .await
                .unwrap();
            let key = pool_keys.data.get(0).unwrap();
            info!("Standard pool key: {:?}", key);
            let ata_a = solana_pubkey::Pubkey::from(
                get_associated_token_address(&owner_pubkey, &Address::from_str_const(&mint_a))
                    .to_bytes(),
            );
            let ata_b = solana_pubkey::Pubkey::from(
                get_associated_token_address(&owner_pubkey, &Address::from_str_const(&mint_b))
                    .to_bytes(),
            );
            println!("ata_a {}", ata_a.to_string());
            let keys = ClmmSwapParams {
                pool_id: solana_pubkey::Pubkey::from_str(&key.id).unwrap(),
                user_input_token: ata_a,
                user_output_token: ata_b,
                amount_specified: amount_in,
                limit_price: None,
                // if false -> amount_in
                base_out: false,
                slippage_bps: 100,
            };

            let _sig = amm_swap_client.swap_clmm(keys).await.unwrap();

            info!(
                "CLMM single‑pool info fetched successfully; CLMM math not yet implemented in this example."
            );
        }
        _ => {}
    }
}
