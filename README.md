## A Rust client library for interacting with Raydium AMM v4 on Solana, providing:
- Fetching pool metadata off-chain via Raydium HTTP API
- Retrieving on-chain reserves directly from Solana RPC
- Computing swap quotes with fee & slippage support
- Executing swap transactions programmatically

## Usage 
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    let rpc_url = env::var("RPC_URL")?;
    let rpc_client = RpcClient::new(rpc_url);
    let mint_a: Pubkey = env::var("MINT_1").unwrap_or_else(|_| SOL_MINT.to_string()).parse()?;
    let mint_b: Pubkey = env::var("MINT_2").unwrap_or_else(|_| SOL_MINT.to_string()).parse()?;
    let keypair = from_bytes_to_key_pair(env::var("KEYPAIR")?);
    let client = AmmSwapClient::new(rpc_client, mint_a, mint_b, keypair);

    // Fetch pool info
    let pools = client.fetch_pool_info("standard", 10, 1).await?;
    println!("Found {} pools", pools.data.len());

    // Pick first pool and compute quote
    let pool = &pools.data[0];
    let onchain = client.get_rpc_pool_info(&pool.id.parse()?).await?;
    let quote = client.compute_amount_out(&onchain, pool, 1_000_000, 0.005)?;
    println!("Quote: {:?}", quote);

    Ok(())
}
```