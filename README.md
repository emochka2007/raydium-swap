## A Rust client library for interacting with Raydium AMM v4 on Solana, providing:
- Fetching pool metadata off-chain via Raydium HTTP API
- Retrieving on-chain reserves directly from Solana RPC
- Computing swap quotes with fee & slippage support
- Executing swap transactions programmatically

## Usage 
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().map_err(|e| anyhow!("Error initializing dotenv {e:?}"))?;
    let url = env::var("RPC_URL")?;
    let mint_1 = env::var("MINT_1").unwrap_or(raydium::SOL_MINT.to_string());
    let mint_2 = env::var("MINT_2")?;
    let rpc_client = RpcClient::new(url);
    let owner = env::var("KEYPAIR").expect("KEYPAIR env is not presented");


    let amount_in = 500;
    let slippage = 0.005;

    let raydium = AmmSwapClient::new(rpc_client, Pubkey::from_str_const(&mint_1), Pubkey::from_str_const(&mint_2), from_bytes_to_key_pair(owner));

    let mut pools = raydium.fetch_pool_info("standard", 100, 1).await.map_err(|e| anyhow!("Error fetching pool_info {e:?}"))?;

    let pool = pools.data.data.get(0).unwrap();

    let pool_id = &pool.id.parse()?;

    let onchain = raydium.get_rpc_pool_info(pool_id).await?;

    let quote = raydium.compute_amount_out(&onchain, pool, amount_in, slippage)?;
    debug!("Quote: {:?}", quote);

    let pool_keys = raydium.fetch_pools_keys_by_id(pool_id).await.map_err(|e|anyhow!("Error fetching pool keys {e:?}"))?;
    debug!("Pool info {pool_keys:?}");

    let key = pool_keys.data.get(0).unwrap();

    let signature = raydium.swap(key, 500, quote.amount_out ).await?;
    info!("Signature: {:?}", signature);

    Ok(())
}
```