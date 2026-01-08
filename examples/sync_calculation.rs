use anyhow::anyhow;
use raydium_amm_swap::amm::client::AmmSwapClient;
use raydium_amm_swap::clmm::calculate_swap_change_accounts;
use raydium_amm_swap::consts::{CLMM, SOL_MINT};
use raydium_amm_swap::helpers::from_bytes_to_key_pair;
use raydium_amm_swap::interface::{AmmPool, ClmmPool, ClmmSwapParams, PoolKeys, PoolType};
use solana_address::Address;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use spl_associated_token_account::get_associated_token_address;
use std::env;
use std::str::FromStr;
use tokio::time::Instant;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    dotenvy::dotenv().unwrap();
    let amount_in = 500_000;
    let slippage = 0.01;
    let url = env::var("RPC_URL").unwrap();
    let mint_a = env::var("MINT_1").unwrap_or(SOL_MINT.to_string());
    let mint_b = env::var("MINT_2").unwrap();
    let rpc_client = RpcClient::new(url);
    let owner = env::var("KEYPAIR").expect("KEYPAIR env is not presented");
    let keypair = from_bytes_to_key_pair(owner);
    let owner_pubkey = keypair.pubkey();
    info!("Owner address {}", owner_pubkey.to_string());
    let amm_swap_client = AmmSwapClient::new(rpc_client, keypair);

    // Choose which kind of pool to query.
    let pool_type = PoolType::Concentrated;

    let all_mint_pools = amm_swap_client
        .fetch_pool_info(&mint_a, &mint_b, &pool_type, Some(100), None, None, None)
        .await
        .unwrap();
    // info!("{:?}", all_mint_pools);

    // For now, compute_amount_out & swap are only wired for standard AMM v4.
    let mint_a = Address::from_str(&mint_a).unwrap();
    let mint_b = Address::from_str(&mint_b).unwrap();
    let first_pool = all_mint_pools.first().unwrap();
    match pool_type {
        PoolType::Standard => {
            let pool_id = Pubkey::from_str(&first_pool.id).unwrap();
            let pool_info = amm_swap_client.fetch_pool_by_id(&pool_id).await.unwrap();
            let pool_keys: PoolKeys<AmmPool> = amm_swap_client
                .fetch_pools_keys_by_id(&pool_id)
                .await
                .unwrap();

            let rpc_data = amm_swap_client
                .get_rpc_pool_info(&pool_id)
                .await
                .map_err(|e| anyhow!("Error fetching rpc pool info {e:?}"))
                .unwrap();
            let pool = pool_info.data.get(0).unwrap();
            let compute = amm_swap_client
                .compute_amount_out(&rpc_data, pool, amount_in, slippage)
                .unwrap();

            let key = pool_keys.data.get(0).unwrap();
            info!("Standard pool key: {:?}", key);

            let signature = amm_swap_client
                .swap_amm(key, &mint_a, &mint_b, amount_in, compute.min_amount_out)
                .await
                .unwrap();
            info!("{signature}");
        }

        PoolType::Concentrated => {
            let pool_id = Pubkey::from_str(&first_pool.id).unwrap();
            let pool_keys: PoolKeys<ClmmPool> = amm_swap_client
                .fetch_pools_keys_by_id(&pool_id)
                .await
                .unwrap();
            let key = pool_keys.data.get(0).unwrap();
            info!("Standard pool key: {:?}", key);
            let ata_a = solana_pubkey::Pubkey::from(
                get_associated_token_address(&owner_pubkey, &mint_a).to_bytes(),
            );
            let ata_b = solana_pubkey::Pubkey::from(
                get_associated_token_address(&owner_pubkey, &mint_b).to_bytes(),
            );
            info!("ata b {}", ata_b.to_string());
            info!("ata_a {}", ata_a.to_string());
            let mint_a = amm_swap_client
                .get_or_create_token_program(&mint_a)
                .await
                .unwrap();

            let mint_b = amm_swap_client
                .get_or_create_token_program(&mint_b)
                .await
                .unwrap();
            let pool_id = solana_pubkey::Pubkey::from_str(&key.id).unwrap();
            let user_input_token = solana_pubkey::Pubkey::from_str(&mint_a.to_string()).unwrap();
            let keys = ClmmSwapParams {
                pool_id: pool_id.clone(),
                user_input_token: user_input_token.clone(),
                user_output_token: solana_pubkey::Pubkey::from_str(&mint_b.to_string()).unwrap(),
                amount_specified: amount_in,
                limit_price: None,
                // if false -> amount is amount_in
                base_out: false,
                slippage_bps: 100,
            };

            let pool_id = Address::from(pool_id.to_bytes());

            let epoch = amm_swap_client.get_epoch().await.unwrap();
            let pool_state = amm_swap_client.get_pool_state(&pool_id).await.unwrap();

            let tick_array_bitmap_extension =
                AmmSwapClient::get_tick_array_bitmap_extension(&pool_id);
            let rsps = amm_swap_client
                .get_rsps(user_input_token, &pool_state, &tick_array_bitmap_extension)
                .await
                .unwrap();
            let base_in = false;
            let params =
                calculate_swap_change_accounts(&rsps, amount_in, pool_state, base_in, epoch)
                    .unwrap();
            let clmm_pubkey = solana_pubkey::Pubkey::from_str_const(CLMM);
            let tick_arrays = amm_swap_client
                .load_cur_and_next_five_tick_array(
                    clmm_pubkey,
                    keys.pool_id,
                    &pool_state,
                    &params.tickarray_bitmap_extension_state,
                    params.zero_for_one,
                )
                .await
                .unwrap();

            let instant = Instant::now();
            info!("Starting swap calc");
            let (_, _) = amm_swap_client
                .calculate_swap_change_clmm_sync(
                    keys,
                    epoch,
                    pool_state,
                    rsps,
                    tick_arrays,
                    tick_array_bitmap_extension,
                )
                .unwrap();

            info!("Finished swap calc ms: {}", instant.elapsed().as_millis());
            info!(
                "Finished swap calc nanosec: {}",
                instant.elapsed().as_nanos()
            );

            // let sig = amm_swap_client
            //     .swap_clmm(ata_b, swap_result, tick_array_pubkey)
            //     .await
            //     .unwrap();
            // info!("{sig}");
        }
    }
}
