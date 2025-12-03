## A Rust client library for interacting with Raydium AMM v4 and CLMM pools on Solana, providing:
- Fetching pool metadata off-chain via the Raydium HTTP API
- Retrieving on-chain reserves directly from Solana RPC
- Computing swap quotes with fee & slippage support
- Executing swap transactions programmatically
- Support for both standard AMM v4 pools and concentrated-liquidity (CLMM) pools via `PoolType::Standard` and `PoolType::Concentrated`.

## Usage

```rust
use std::env;
use std::str::FromStr;

use anyhow::anyhow;
use raydium_amm_swap::amm::client::AmmSwapClient;
use raydium_amm_swap::consts::SOL_MINT;
use raydium_amm_swap::helpers::from_bytes_to_key_pair;
use raydium_amm_swap::interface::{AmmPool, PoolKeys, PoolType};
use solana_address::Address;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration from environment.
    dotenvy::dotenv().ok();

    let amount_in: u64 = 500;
    let slippage: f64 = 0.005; // 0.5%

    let url = env::var("RPC_URL")?;
    let mint_a_str = env::var("MINT_1").unwrap_or(SOL_MINT.to_string());
    let mint_b_str = env::var("MINT_2")?;

    let rpc_client = RpcClient::new(url);
    let owner = env::var("KEYPAIR").expect("KEYPAIR env is not presented");
    let keypair = from_bytes_to_key_pair(owner);

    let client = AmmSwapClient::new(rpc_client, keypair);

    // In this example we request a standard AMM v4 pool.
    // You can also pass `PoolType::Concentrated` to work with CLMM pools.
    let pool_type = PoolType::Standard;

    let pools = client
        .fetch_pool_info(&mint_a_str, &mint_b_str, &pool_type, Some(1), None, None, None)
        .await?;

    if pools.is_empty() {
        return Err(anyhow!("no matching pools found"));
    }

    // Choose the first matching pool (works for both standard and CLMM).
    let first_pool = &pools[0];
    let pool_id_str = &first_pool.id;

    let pool_id = Pubkey::from_str(pool_id_str)?;

    // Fetch on-chain reserves and compute a quote.
    let rpc_pool = client.get_rpc_pool_info(&pool_id).await?;
    let pool_info = client.fetch_pool_by_id(&pool_id).await?;
    let pool = pool_info.data.get(0).expect("no pool data");

    let quote = client.compute_amount_out(&rpc_pool, pool, amount_in, slippage)?;
    debug!("Quote: {:?}", quote);

    // Fetch full pool keys for building the swap instruction.
    let pool_keys: PoolKeys<AmmPool> = client.fetch_pools_keys_by_id(&pool_id).await?;
    let key = pool_keys.data.get(0).expect("missing pool keys");

    // Convert mint strings into `Address` for the client.
    let mint_a = Address::from_str_const(&mint_a_str);
    let mint_b = Address::from_str_const(&mint_b_str);

    // IMPORTANT: pass `min_amount_out` to enforce slippage on-chain.
    let signature = client
        .swap_amm(key, &mint_a, &mint_b, amount_in, quote.min_amount_out, None)
        .await?;
    info!("Swap signature: {:?}", signature);

    Ok(())
}
```

The client will automatically:

- Create associated token accounts for the owner if they do not exist.
- Wrap SOL into wSOL only when the mint is the native SOL mint.
- Enforce your slippage tolerance when you pass `quote.min_amount_out` into `swap_amm`.
