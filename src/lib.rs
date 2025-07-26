//! A high‑level client for interacting with Raydium AMM pools on Solana.
//!
//! This crate provides:
//! - Retrieval of on‑chain and off‑chain pool data (`fetch_pool_info`, `fetch_pool_by_id`, etc.)
//! - Computation of swap quotes with fee and slippage handling (`compute_amount_out`).
//! - Execution of swaps against a given pool (`swap`).
//!
//! # Examples
//!
//! ```rust
//!
//! use std::env;
//! use std::str::FromStr;
//! use anchor_lang::prelude::Pubkey;
//! use anyhow::anyhow;
//!
//! use solana_client::rpc_client::RpcClient;
//!
//! #[tokio::test]
//! async fn test_swap() {
//!     let amount_in = 500;
//!     let slippage = 0.01;
//!     let url = env::var("RPC_URL").unwrap();
//!     let mint_1 = Pubkey::from_str(&env::var("MINT_1").unwrap()).unwrap();
//!     let mint_2 = Pubkey::from_str(&env::var("MINT_2").unwrap()).unwrap();
//!     let rpc_client = RpcClient::new(url);
//!     let owner = env::var("KEYPAIR").expect("KEYPAIR env is not presented");
//!     let raydium = AmmSwapClient::new(rpc_client, mint_1, mint_2, from_bytes_to_key_pair(owner));
//!
//!     let pool_id = Pubkey::from_str_const("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2");
//!
//!     let pool_info = amm_swap_client.fetch_pool_by_id(&pool_id).await.map_err(|e| anyhow!("Error fetching pool by id {e:?}")).unwrap();
//!
//!     let pool_keys = amm_swap_client.fetch_pools_keys_by_id(&pool_id).await.map_err(|e|anyhow!("Error fetching pool keys {e:?}")).unwrap();
//!
//!     let rpc_data = amm_swap_client.get_rpc_pool_info(&pool_id).await.map_err(|e|anyhow!("Error fetching rpc pool info {e:?}")).unwrap();
//!
//!     let pool = pool_info.data.get(0).unwrap();
//!
//!     let compute = amm_swap_client
//!         .compute_amount_out(
//!             &rpc_data,
//!             &pool,
//!             amount_in,
//!             0.01,
//!         ).unwrap();
//!
//!     let key = pool_keys.data.get(0).unwrap();
//!
//!     let _sig = amm_swap_client
//!         .swap(
//!             key,
//!             amount_in,
//!             compute.amount_out,
//!         )
//!         .await.unwrap();
//!     assert!(true);
//!
//! }
//! ```
pub mod amm;
pub mod consts;
pub mod helpers;
pub mod interface;
pub mod raydium;
