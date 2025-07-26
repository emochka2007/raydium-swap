//! A high‑level client for interacting with Raydium AMM pools on Solana.
//!
//! This crate provides:
//! - Retrieval of on‑chain and off‑chain pool data (`fetch_pool_info`, `fetch_pool_by_id`, etc.)
//! - Computation of swap quotes with fee and slippage handling (`compute_amount_out`).
//! - Execution of swaps against a given pool (`swap`).
//!
//! # Examples
//!
//! ```no_run
//! use raydium_amm_swap::AmmSwapClient;
//! # // initialize with real RPC URL and keypair...
//! # let client = AmmSwapClient::new(...);
//! let pool_id = "...".parse().unwrap();
//! let info = client.fetch_pool_by_id(&pool_id).await.unwrap();
//! let rpc = client.get_rpc_pool_info(&pool_id).await.unwrap();
//! let quote = client.compute_amount_out(&rpc, &info.data[0], 1_000_000, 0.005).unwrap();
//! println!("You’ll get at least {} tokens", quote.min_amount_out);
//! ```
mod consts;
mod raydium;
mod amm;
mod interface;
mod helpers;
