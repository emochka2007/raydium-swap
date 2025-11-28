use crate::amm::{AmmInstruction, SwapInstructionBaseIn};
use crate::consts::{AMM_V4, LIQUIDITY_FEES_DENOMINATOR, LIQUIDITY_FEES_NUMERATOR};
use crate::interface::{
    PoolInfoData, PoolInfoResponse, PoolInfosResponse, PoolKey, PoolKeysResponse,
};
use anyhow::{Context, anyhow};
use borsh::{BorshDeserialize, BorshSerialize};
use reqwest::Client;
use serde::de::DeserializeOwned;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use solana_system_interface::instruction::transfer;
use tracing::log::info;
use tracing::{debug, warn};

/// The result of computing a swap quote.
#[derive(Debug)]
pub struct ComputeAmountOutResult {
    /// Raw amount out before slippage.
    pub amount_out: u64,
    /// Minimum amount out after slippage tolerance.
    pub min_amount_out: u64,
    /// Current on‑chain price (quote/base).
    pub current_price: f64,
    /// Execution price for the quoted trade.
    pub execution_price: f64,
    /// Percent price impact of this trade.
    pub price_impact: f64,
    /// Fee deducted from the input.
    pub fee: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct LiquidityStateLayoutV4 {
    pub status: u64,
    pub nonce: u64,
    pub max_order: u64,
    pub depth: u64,
    pub base_decimal: u64,
    pub quote_decimal: u64,
    pub state: u64,
    pub reset_flag: u64,
    pub min_size: u64,
    pub vol_max_cut_ratio: u64,
    pub amount_wave_ratio: u64,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub min_price_multiplier: u64,
    pub max_price_multiplier: u64,
    pub system_decimal_value: u64,
    pub min_separate_numerator: u64,
    pub min_separate_denominator: u64,
    pub trade_fee_numerator: u64,
    pub trade_fee_denominator: u64,
    pub pnl_numerator: u64,
    pub pnl_denominator: u64,
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
    pub base_need_take_pnl: u64,
    pub quote_need_take_pnl: u64,
    pub quote_total_pnl: u64,
    pub base_total_pnl: u64,
    pub pool_open_time: u64,
    pub punish_pc_amount: u64,
    pub punish_coin_amount: u64,
    pub orderbook_to_init_time: u64,
    pub swap_base_in_amount: u128,
    pub swap_quote_out_amount: u128,
    pub swap_base2quote_fee: u64,
    pub swap_quote_in_amount: u128,
    pub swap_base_out_amount: u128,
    pub swap_quote2base_fee: u64,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub open_orders: Pubkey,
    pub market_id: Pubkey,
    pub market_program_id: Pubkey,
    pub target_orders: Pubkey,
    pub withdraw_queue: Pubkey,
    pub lp_vault: Pubkey,
    pub owner: Pubkey,
    pub lp_reserve: u64,
    pub padding: [u64; 3],
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
struct AccountLayout {
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate_option: u32,
    delegate: Pubkey,
    state: u8,
    is_native_option: u32,
    is_native: u64,
    delegated_amount: u64,
    close_authority_option: u32,
    close_authority: Pubkey,
}

/// On‑chain reserves for a pool.
pub struct RpcPoolInfo {
    /// Amount of quote token in vault.
    pub quote_reserve: u64,
    /// Amount of base token in vault.
    pub base_reserve: u64,
}

/// High‑level client for performing swaps between two mints.
pub struct AmmSwapClient {
    reqwest_client: Client,
    base_url: String,
    owner: Keypair,
    rpc_client: RpcClient,
    mint_1: Pubkey,
    mint_2: Pubkey,
}

impl AmmSwapClient {
    /// Creates a new swap client.
    ///
    /// # Arguments
    ///
    /// - `rpc_client`: the Solana RPC client to use.
    /// - `mint_1`: the base token mint.
    /// - `mint_2`: the quote token mint.
    /// - `owner`: signer for transaction execution.
    pub fn new(rpc_client: RpcClient, mint_1: Pubkey, mint_2: Pubkey, owner: Keypair) -> Self {
        let reqwest_client = Client::new();
        let base_url = "https://api-v3.raydium.io".to_string();
        Self {
            rpc_client,
            base_url,
            mint_1,
            mint_2,
            owner,
            reqwest_client,
        }
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: Option<&str>,
        query: Option<&[(&str, &str)]>,
    ) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path.unwrap_or_default());
        let response = self
            .reqwest_client
            .get(&url)
            .query(query.unwrap_or(&[]))
            .send()
            .await
            .context("Raydium amm get failed")?
            .error_for_status()
            .context("Raydium non-200")?;

        Ok(response.json::<T>().await?)
    }

    /// Fetch raw pool account keys by pool ID via HTTP API.
    pub async fn fetch_pools_keys_by_id(&self, id: &Pubkey) -> anyhow::Result<PoolKeysResponse> {
        let id = id.to_string();
        let headers = ("ids", id.as_str());
        let resp: PoolKeysResponse = self.get(Some("/pools/key/ids"), Some(&[headers])).await?;
        Ok(resp)
    }

    /// Retrieve on‑chain reserves for a given pool account.
    ///
    /// # Errors
    /// Returns an error if the account data cannot be deserialized.
    pub async fn get_rpc_pool_info(&self, pool_id: &Pubkey) -> anyhow::Result<RpcPoolInfo> {
        let account = self.rpc_client.get_account(pool_id).await?;
        let data = account.data;
        let market_state = LiquidityStateLayoutV4::try_from_slice(&data)
            .map_err(|e| anyhow!("Failed to decode market state: {:?}", e))?;
        debug!("Market state {:?}", market_state);
        let mint1_account_data = self
            .rpc_client
            .get_account_with_commitment(&market_state.base_vault, CommitmentConfig::confirmed())
            .await?
            .value
            .ok_or(anyhow!("mint1 Account Data Value not found"))?;
        let mint2_account_data = self
            .rpc_client
            .get_account_with_commitment(&market_state.quote_vault, CommitmentConfig::confirmed())
            .await?
            .value
            .ok_or(anyhow!("mint2 Account Data Value not found"))?;

        let mint_1_layout = AccountLayout::try_from_slice(&mint1_account_data.data)?;
        let mint_2_layout = AccountLayout::try_from_slice(&mint2_account_data.data)?;
        let base_reserve = mint_1_layout.amount - market_state.base_need_take_pnl;
        let quote_reserve = mint_2_layout.amount - market_state.quote_need_take_pnl;
        Ok(RpcPoolInfo {
            base_reserve,
            quote_reserve,
        })
    }

    /// Fetch pool metadata (price, TVL, stats) by ID via HTTP API.
    pub async fn fetch_pool_by_id(&self, id: &Pubkey) -> anyhow::Result<PoolInfoResponse> {
        let id = id.to_string();
        let headers = ("ids", id.as_str());
        self.get(Some("/pools/info/ids"), Some(&[headers])).await
    }

    /// List pools for the given pair via HTTP API.
    ///
    /// - `pool_type`: e.g. "standard".
    /// - `page_size`, `page`: pagination.
    pub async fn fetch_pool_info(
        &self,
        pool_type: &str,
        page_size: u32,
        page: u32,
    ) -> anyhow::Result<PoolInfosResponse> {
        let pool_sort_field = "default";
        let sort_type = "desc";
        let url = format!(
            "https://api-v3.raydium.io/pools/info/mint?mint1={}&mint2={}&poolType={}&poolSortField={}&sortType={}&pageSize={}&page={}",
            self.mint_1, self.mint_2, pool_type, pool_sort_field, sort_type, page_size, page
        );
        let client = Client::new();
        let resp = client.get(url).send().await?;
        Ok(resp.json().await?)
    }

    /// Compute a swap quote (amount out, fee, slippage).
    ///
    /// # Arguments
    ///
    /// - `rpc_pool_info`: on‑chain reserves.
    /// - `pool_info`: off‑chain pool metadata.
    /// - `amount_in`: amount of base token to swap (in smallest units).
    /// - `slippage`: tolerance (e.g. `0.005` for 0.5%).
    pub fn compute_amount_out(
        &self,
        rpc_pool_info: &RpcPoolInfo,
        pool_info: &PoolInfoData,
        amount_in: u64,
        slippage: f64,
    ) -> anyhow::Result<ComputeAmountOutResult> {
        let reserve_in = rpc_pool_info.base_reserve;
        let reserve_out = rpc_pool_info.quote_reserve;
        debug!("Reserve out: {}", reserve_out);
        debug!("Reserve in: {}", reserve_in);

        let mint_in_decimals = pool_info.mint_a.decimals;
        let mint_out_decimals = pool_info.mint_b.decimals;

        let div_in = 10u128.pow(mint_in_decimals);
        let div_out = 10u128.pow(mint_out_decimals);

        let reserve_in_f = reserve_in as f64 / div_in as f64;
        let reserve_out_f = reserve_out as f64 / div_out as f64;

        // ------- Current price calculation ---------
        let current_price = reserve_out_f / reserve_in_f;
        debug!("Current price {}", current_price);

        // ------- Amount + Fee calculation --------
        let fee = amount_in
            .saturating_mul(LIQUIDITY_FEES_NUMERATOR)
            .div_ceil(LIQUIDITY_FEES_DENOMINATOR);
        let amount_in_with_fee = amount_in.saturating_sub(fee);
        let denominator = reserve_in.saturating_add(amount_in_with_fee);
        let amount_out_raw = reserve_out.saturating_mul(amount_in_with_fee) / denominator;

        let min_amount_out = ((amount_out_raw as f64) * (1.0 - slippage)).floor() as u64;

        let exec_out_f = min_amount_out as f64 / div_out as f64;
        let exec_in_f = amount_in.saturating_sub(fee) as f64 / div_in as f64;
        let execution_price = exec_out_f / exec_in_f;

        let price_impact = (current_price - execution_price) / current_price * 100.0;

        debug!("Price impact {price_impact}");

        Ok(ComputeAmountOutResult {
            amount_out: amount_out_raw,
            min_amount_out,
            current_price,
            execution_price,
            price_impact,
            fee,
        })
    }

    async fn get_or_create_token_program(&self, mint: Pubkey) -> anyhow::Result<Pubkey> {
        let associated_token_account =
            spl_associated_token_account::get_associated_token_address(&self.owner.pubkey(), &mint);
        let balance = self
            .rpc_client
            .get_token_account_balance(&associated_token_account)
            .await;
        match balance {
            Ok(balance) => {
                debug!(
                    "Address {:?}, balance {:?}",
                    associated_token_account, balance
                );
                return Ok(associated_token_account);
            }
            Err(e) => {
                warn!(
                    "Error fetching balance Address {:?}, e {:?}",
                    associated_token_account, e
                );
                let instructions = vec![
                    spl_associated_token_account::instruction::create_associated_token_account(
                        &self.owner.pubkey(),
                        &self.owner.pubkey(),
                        &spl_token::native_mint::id(),
                        &spl_token::id(),
                    ),
                    // Amount is hardcoded based on network fee
                    transfer(&self.owner.pubkey(), &associated_token_account, 2_500_000),
                    spl_token::instruction::sync_native(
                        &spl_token::id(),
                        &associated_token_account,
                    )?,
                ];

                let recent_blockhash: solana_sdk::hash::Hash =
                    self.rpc_client.get_latest_blockhash().await?;
                let transaction = Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&self.owner.pubkey()),
                    &[&self.owner],
                    recent_blockhash,
                );
                let sig = self
                    .rpc_client
                    .send_and_confirm_transaction_with_spinner(&transaction)
                    .await?;

                info!("SOL wrapped {:?}", sig);
            }
        }

        Ok(associated_token_account)
    }

    /// Swap coin or pc from pool, base amount_in with a slippage of minimum_amount_out
    ///
    ///   0. `[]` Spl Token program id
    ///   1. `[writable]` AMM Account
    ///   2. `[]` $authority derived from `create_program_address(&[AUTHORITY_AMM, &[nonce]])`.
    ///   3. `[writable]` AMM open orders Account
    ///   4. `[writable]` (optional)AMM target orders Account, no longer used in the contract, recommended no need to add this Account.
    ///   5. `[writable]` AMM coin vault Account to swap FROM or To.
    ///   6. `[writable]` AMM pc vault Account to swap FROM or To.
    ///   7. `[]` Market program id
    ///   8. `[writable]` Market Account. Market program is the owner.
    ///   9. `[writable]` Market bids Account
    ///   10. `[writable]` Market asks Account
    ///   11. `[writable]` Market event queue Account
    ///   12. `[writable]` Market coin vault Account
    ///   13. `[writable]` Market pc vault Account
    ///   14. '[]` Market vault signer Account
    ///   15. `[writable]` User source token Account.
    ///   16. `[writable]` User destination token Account.
    ///   17. `[signer]` User wallet Account
    pub async fn swap(
        &self,
        pool_keys: &PoolKey,
        amount_in: u64,
        amount_out: u64, // out.amount_out means amount 'without' slippage
    ) -> anyhow::Result<Signature> {
        let amm_program = Pubkey::from_str_const(AMM_V4);

        let user_token_source = self.get_or_create_token_program(self.mint_1).await?;
        let user_token_destination = self.get_or_create_token_program(self.mint_2).await?;

        info!(
            "Executing swap from {:?} to {:?}",
            user_token_source, user_token_destination
        );

        let data = AmmInstruction::SwapBaseIn(SwapInstructionBaseIn {
            amount_in,
            minimum_amount_out: amount_out,
        })
        .pack()?;

        let accounts = vec![
            // spl token
            AccountMeta::new_readonly(spl_token::id(), false),
            // amm
            AccountMeta::new(pool_keys.id.parse()?, false),
            AccountMeta::new_readonly(pool_keys.authority.parse()?, false),
            AccountMeta::new(pool_keys.open_orders.parse()?, false),
            // AccountMeta::new(*amm_target_orders, false),
            AccountMeta::new(pool_keys.vault.a.parse()?, false),
            AccountMeta::new(pool_keys.vault.b.parse()?, false),
            // market
            AccountMeta::new_readonly(pool_keys.market_program_id.parse()?, false),
            AccountMeta::new(pool_keys.market_id.parse()?, false),
            AccountMeta::new(pool_keys.market_bids.parse()?, false),
            AccountMeta::new(pool_keys.market_asks.parse()?, false),
            AccountMeta::new(pool_keys.market_event_queue.parse()?, false),
            AccountMeta::new(pool_keys.market_base_vault.parse()?, false),
            AccountMeta::new(pool_keys.market_quote_vault.parse()?, false),
            AccountMeta::new(pool_keys.market_authority.parse()?, false),
            // user
            AccountMeta::new(user_token_source, false),
            AccountMeta::new(user_token_destination, false),
            AccountMeta::new_readonly(self.owner.pubkey(), true),
        ];

        let ix = Instruction {
            program_id: amm_program,
            accounts,
            data,
        };
        let recent_blockhash = &self.rpc_client.get_latest_blockhash().await?;

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.owner.pubkey()),
            &[&self.owner],
            *recent_blockhash,
        );

        let sig = &self.rpc_client.send_and_confirm_transaction(&tx).await?;
        info!("Executed with Signature {sig}");
        Ok(*sig)
    }
}
