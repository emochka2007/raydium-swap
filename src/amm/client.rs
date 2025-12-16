use crate::amm::{AmmInstruction, SwapInstructionBaseIn};
use crate::clmm::{
    ClmmSwapChangeResult, clmm_utils, clmm_utils_sync, get_tick_array_keys, get_tick_arrays,
};
use crate::common::rpc;
use crate::consts::{
    AMM_V4, CLMM, LIQUIDITY_FEES_DENOMINATOR, LIQUIDITY_FEES_NUMERATOR, swap_v2_discriminator,
};
use crate::interface::{
    AmmPool, ClmmPool, ClmmPoolInfosResponse, ClmmSinglePoolInfo, ClmmSwapParams, PoolKeys,
    PoolType, Rsps, TickArrays,
};
use crate::states::{POOL_TICK_ARRAY_BITMAP_SEED, PoolState, TickArrayBitmapExtension};
use anchor_lang::prelude::{Rent, SolanaSysvar};
use anchor_spl::memo::spl_memo;
use anyhow::{Context, anyhow};
use borsh::{BorshDeserialize, BorshSerialize};
use log::warn;
use reqwest::Client;
use serde::de::DeserializeOwned;
use solana_address::Address;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use solana_system_interface::instruction::transfer;
use spl_token::solana_program::program_pack::Pack;
use tracing::log::info;
use tracing::{debug, error};

/// The result of computing a swap quote.
#[derive(Debug, Clone)]
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

/// The result of computing the required input amount for a desired output.
#[derive(Debug, Clone)]
pub struct ComputeAmountInResult {
    /// Raw amount in before slippage.
    pub amount_in: u64,
    /// Maximum amount in after slippage tolerance.
    pub max_amount_in: u64,
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

#[cfg_attr(feature = "derive", derive(Debug))]
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
    pub fn new(rpc_client: RpcClient, owner: Keypair) -> Self {
        Self::new_with_base_url(rpc_client, owner, "https://api-v3.raydium.io")
    }

    /// Creates a new swap client with a custom Raydium HTTP base URL.
    ///
    /// This is primarily useful for testing or pointing at alternate
    /// Raydium API environments.
    pub fn new_with_base_url(
        rpc_client: RpcClient,
        owner: Keypair,
        base_url: impl Into<String>,
    ) -> Self {
        let reqwest_client = Client::new();
        Self {
            rpc_client,
            base_url: base_url.into(),
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

        let resp = self
            .reqwest_client
            .get(&url)
            .query(query.unwrap_or(&[]))
            .send()
            .await
            .with_context(|| format!("Raydium AMM GET failed for {}", url))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .with_context(|| format!("Failed to read response body from {}", url))?;

        if !status.is_success() {
            error!("Raydium non-200 {} for {}. Body: {}", status, url, body);
            anyhow::bail!("Raydium non-200 {} for {}", status, url);
        }

        debug!("Raydium response body for {}: {}", url, body);

        let parsed: T = serde_json::from_str(&body).with_context(|| {
            format!(
                "Failed to parse Raydium response as JSON. Status: {}, Body: {}",
                status, body
            )
        })?;

        Ok(parsed)
    }

    pub fn owner_pubkey(&self) -> Pubkey {
        self.owner.pubkey()
    }

    /// Fetch raw pool account keys by pool ID via HTTP API.
    pub async fn fetch_pools_keys_by_id<T: DeserializeOwned + Clone>(
        &self,
        id: &Pubkey,
    ) -> anyhow::Result<PoolKeys<T>> {
        let id = id.to_string();
        let headers = ("ids", id.as_str());
        let resp: PoolKeys<T> = self.get(Some("/pools/key/ids"), Some(&[headers])).await?;
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
    pub async fn fetch_pool_by_id(&self, id: &Pubkey) -> anyhow::Result<ClmmSinglePoolInfo> {
        let id = id.to_string();
        let headers = ("ids", id.as_str());
        let resp: ClmmSinglePoolInfo = self.get(Some("/pools/info/ids"), Some(&[headers])).await?;
        Ok(resp)
    }

    /// List pools for the given pair via HTTP API.
    ///
    /// - `pool_type`: e.g. "standard".
    /// - `page_size`, `page`: pagination.
    pub async fn fetch_pool_info(
        &self,
        mint_a: &str,
        mint_b: &str,
        pool_type: &PoolType,
        page_size: Option<u32>,
        page: Option<u32>,
        pool_sort_field: Option<&str>,
        sort_type: Option<&str>,
    ) -> anyhow::Result<Vec<ClmmPool>> {
        let page_size_str = page_size.unwrap_or(100).to_string();
        let page_str = page.unwrap_or(1).to_string();
        let pool_type_str = pool_type.to_string();
        let pool_sort_field = pool_sort_field.unwrap_or("default");
        let sort_type = sort_type.unwrap_or("desc");
        let headers = [
            ("mint1", mint_a),
            ("mint2", mint_b),
            ("poolType", pool_type_str.as_str()),
            ("poolSortField", pool_sort_field),
            ("sortType", sort_type),
            ("pageSize", page_size_str.as_str()),
            ("page", page_str.as_str()),
        ];
        let resp: ClmmPoolInfosResponse =
            self.get(Some("/pools/info/mint"), Some(&headers)).await?;
        let mut parsed_pools = Vec::new();
        for pool in &resp.data.data {
            match serde_json::from_value::<ClmmPool>(pool.clone()) {
                Ok(parsed_pool) => parsed_pools.push(parsed_pool),
                Err(e) => {
                    warn!(
                        "Encountered non amm/clmm pool: error={}, programId={:?}",
                        e,
                        pool.get("id")
                    );
                    continue;
                }
            }
        }

        // Filter pools to only have program_id = AMM and CLMM
        let filtered_pools = parsed_pools
            .iter()
            .filter(|pool| pool.program_id == AMM_V4 || pool.program_id == CLMM)
            .cloned()
            .collect();

        Ok(filtered_pools)
    }

    /// Compute a swap quote (amount out, fee, slippage).
    ///
    /// # Arguments
    ///
    /// - `rpc_pool_info`: on‑chain reserves.
    /// - `pool_info`: off‑chain pool metadata.
    /// - `amount_in`: amount of base token to swap (in the smallest units).
    /// - `slippage`: tolerance (e.g. `0.005` for 0.5%).
    pub fn compute_amount_out(
        &self,
        rpc_pool_info: &RpcPoolInfo,
        pool_info: &ClmmPool,
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

    /// Compute the required swap input (amount in, fee, slippage).
    ///
    /// This is the inverse of [`compute_amount_out`]: it finds the smallest
    /// input amount such that the pool would output at least `amount_out`
    /// (before applying the slippage tolerance), using the same constant
    /// product curve and on-chain fee logic.
    ///
    /// # Arguments
    ///
    /// - `rpc_pool_info`: on‑chain reserves.
    /// - `pool_info`: off‑chain pool metadata.
    /// - `amount_out`: desired amount of quote token to receive (in the smallest units).
    /// - `slippage`: tolerance (e.g. `0.005` for 0.5%).
    pub fn compute_amount_in(
        &self,
        rpc_pool_info: &RpcPoolInfo,
        pool_info: &ClmmPool,
        amount_out: u64,
        slippage: f64,
    ) -> anyhow::Result<ComputeAmountInResult> {
        let reserve_in = rpc_pool_info.base_reserve;
        let reserve_out = rpc_pool_info.quote_reserve;

        if amount_out == 0 {
            return Err(anyhow!("amount_out must be greater than zero"));
        }

        if amount_out >= reserve_out {
            return Err(anyhow!(
                "requested amount_out {} exceeds pool reserve {}",
                amount_out,
                reserve_out
            ));
        }

        // Ensure the target output is achievable with current liquidity by
        // checking the extreme case of consuming up to all available input.
        let max_input = reserve_in;
        let max_quote = self.compute_amount_out(rpc_pool_info, pool_info, max_input, 0.0)?;
        if max_quote.amount_out < amount_out {
            return Err(anyhow!(
                "requested amount_out {} cannot be satisfied by pool liquidity (max reachable {})",
                amount_out,
                max_quote.amount_out
            ));
        }

        // Binary-search the minimal amount_in that yields at least amount_out,
        // using the same math as `compute_amount_out` so rounding and fees
        // stay consistent.
        let mut low: u64 = 1;
        let mut high: u64 = max_input;
        let mut required_in: u64 = max_input;

        while low <= high {
            let mid = low + (high - low) / 2;
            let quote = self.compute_amount_out(rpc_pool_info, pool_info, mid, 0.0)?;

            if quote.amount_out >= amount_out {
                required_in = mid;
                if mid == 0 {
                    break;
                }
                if mid == 1 {
                    break;
                }
                high = mid.saturating_sub(1);
            } else {
                low = mid.saturating_add(1);
            }
        }

        let fee = required_in
            .saturating_mul(LIQUIDITY_FEES_NUMERATOR)
            .div_ceil(LIQUIDITY_FEES_DENOMINATOR);
        let amount_in_with_fee = required_in.saturating_sub(fee);

        let mint_in_decimals = pool_info.mint_a.decimals;
        let mint_out_decimals = pool_info.mint_b.decimals;

        let div_in = 10u128.pow(mint_in_decimals);
        let div_out = 10u128.pow(mint_out_decimals);

        let reserve_in_f = reserve_in as f64 / div_in as f64;
        let reserve_out_f = reserve_out as f64 / div_out as f64;

        // ------- Current price calculation ---------
        let current_price = reserve_out_f / reserve_in_f;

        // ------- Execution price and impact -------
        let exec_out_f = amount_out as f64 / div_out as f64;
        let exec_in_f = amount_in_with_fee as f64 / div_in as f64;
        let execution_price = exec_out_f / exec_in_f;

        let price_impact = (current_price - execution_price) / current_price * 100.0;

        let max_amount_in = ((required_in as f64) * (1.0 + slippage)).ceil() as u64;

        Ok(ComputeAmountInResult {
            amount_in: required_in,
            max_amount_in,
            current_price,
            execution_price,
            price_impact,
            fee,
        })
    }

    pub async fn get_or_create_token_program(
        &self,
        mint: &Pubkey,
        lamports_fee: Option<u64>,
    ) -> anyhow::Result<Pubkey> {
        let associated_token_account =
            spl_associated_token_account::get_associated_token_address(&self.owner.pubkey(), mint);
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
                let mut instructions = vec![
                    spl_associated_token_account::instruction::create_associated_token_account(
                        &self.owner.pubkey(),
                        &self.owner.pubkey(),
                        mint,
                        // Could be a potential bug with the spl_token2022 ata
                        &spl_token::id(),
                    ),
                ];

                // For the native SOL mint, optionally wrap lamports into wSOL by
                // transferring lamports and calling `sync_native`. For arbitrary SPL
                // mints we only create the associated token account.
                if *mint == spl_token::native_mint::id() {
                    let rent = Rent::get()?;
                    let amount_to_wrap = lamports_fee
                        .unwrap_or(rent.minimum_balance(spl_token::state::Account::LEN));
                    instructions.push(transfer(
                        &self.owner.pubkey(),
                        &associated_token_account,
                        amount_to_wrap,
                    ));
                    instructions.push(spl_token::instruction::sync_native(
                        &spl_token::id(),
                        &associated_token_account,
                    )?);
                }

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

                if *mint == spl_token::native_mint::id() {
                    info!("SOL wrapped {:?}", sig);
                } else {
                    info!("Created associated token account {:?}", sig);
                }
            }
        }

        Ok(associated_token_account)
    }

    pub async fn swap_amm(
        &self,
        pool_keys: &AmmPool,
        mint_a: &Address,
        mint_b: &Address,
        amount_in: u64,
        amount_out: u64, // out.amount_out means amount 'without' slippage
        create_program_fee: Option<u64>,
    ) -> anyhow::Result<Signature> {
        let amm_program = Pubkey::from_str_const(AMM_V4);

        let user_token_source = self
            .get_or_create_token_program(mint_a, create_program_fee)
            .await?;
        let user_token_destination = self
            .get_or_create_token_program(mint_b, create_program_fee)
            .await?;

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

        self.send_and_sign_transaction(&[ix]).await
    }

    async fn send_and_sign_transaction(&self, ix: &[Instruction]) -> anyhow::Result<Signature> {
        let recent_blockhash = &self.rpc_client.get_latest_blockhash().await?;

        let tx = Transaction::new_signed_with_payer(
            ix,
            Some(&self.owner.pubkey()),
            &[&self.owner],
            *recent_blockhash,
        );

        let sig = &self.rpc_client.send_and_confirm_transaction(&tx).await?;
        info!("Executed with Signature {sig}");
        Ok(*sig)
    }

    pub async fn calculate_swap_change_clmm(
        &self,
        params: ClmmSwapParams,
    ) -> anyhow::Result<(ClmmSwapChangeResult, solana_pubkey::Pubkey)> {
        let base_in = !params.base_out;
        let tickarray_bitmap_extension = Pubkey::find_program_address(
            &[
                POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
                params.pool_id.to_bytes().as_ref(),
            ],
            &Pubkey::from_str_const(CLMM),
        )
        .0;

        let tickarray_bitmap_extension =
            solana_pubkey::Pubkey::from(tickarray_bitmap_extension.to_bytes());

        let clmm_pubkey = solana_pubkey::Pubkey::from_str_const(CLMM);

        // todo add sync
        let result = clmm_utils::calculate_swap_change(
            &self.rpc_client,
            clmm_pubkey,
            params.pool_id,
            tickarray_bitmap_extension,
            params.user_input_token,
            params.amount_specified,
            params.limit_price,
            base_in,
            params.slippage_bps,
        )
        .await?;
        Ok((result, tickarray_bitmap_extension))
    }

    pub async fn get_epoch(&self) -> anyhow::Result<u64> {
        Ok(self.rpc_client.get_epoch_info().await?.epoch)
    }

    pub async fn get_pool_state(&self, pool_id: &Pubkey) -> anyhow::Result<PoolState> {
        rpc::get_anchor_account::<PoolState>(&self.rpc_client, &pool_id)
            .await?
            .ok_or(anyhow!("Pool state was not found by rpc"))
    }

    pub async fn get_rsps(
        &self,
        input_token: solana_pubkey::Pubkey,
        pool_state: &PoolState,
        tickarray_bitmap_extension: &solana_pubkey::Pubkey,
    ) -> anyhow::Result<Rsps> {
        let load_accounts: Vec<Address> = [
            input_token,
            pool_state.amm_config,
            pool_state.token_mint_0,
            pool_state.token_mint_1,
            tickarray_bitmap_extension.clone(),
        ]
        .iter()
        .map(|pubkey| Address::from(pubkey.to_bytes()))
        .collect();

        Ok(self
            .rpc_client
            .get_multiple_accounts(&load_accounts)
            .await?)
    }

    pub fn get_tick_array_bitmap_extension(pool_id: &Address) -> solana_pubkey::Pubkey {
        let tickarray_bitmap_extension = Pubkey::find_program_address(
            &[
                POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
            ],
            &Pubkey::from_str_const(CLMM),
        )
        .0;

        let tickarray_bitmap_extension =
            solana_pubkey::Pubkey::from(tickarray_bitmap_extension.to_bytes());

        tickarray_bitmap_extension
    }

    pub async fn load_cur_and_next_five_tick_array(
        &self,
        raydium_v3_program: solana_pubkey::Pubkey,
        pool_id: solana_pubkey::Pubkey,
        pool_state: &PoolState,
        tickarray_bitmap_extension: &TickArrayBitmapExtension,
        zero_for_one: bool,
    ) -> anyhow::Result<TickArrays> {
        let tick_array_keys = get_tick_array_keys(
            raydium_v3_program,
            pool_id,
            pool_state,
            tickarray_bitmap_extension,
            zero_for_one,
        )?;
        let tick_array_rsps =
            clmm_utils::get_tick_array_rsps(&self.rpc_client, &tick_array_keys).await?;
        get_tick_arrays(tick_array_rsps)
    }

    pub fn calculate_swap_change_clmm_sync(
        &self,
        params: ClmmSwapParams,
        epoch: u64,
        pool_state: PoolState,
        rsps: Rsps,
        tick_arrays: TickArrays,
        tickarray_bitmap_extension: solana_pubkey::Pubkey,
    ) -> anyhow::Result<(ClmmSwapChangeResult, solana_pubkey::Pubkey)> {
        let base_in = !params.base_out;

        let clmm_pubkey = solana_pubkey::Pubkey::from_str_const(CLMM);

        let result = clmm_utils_sync::calculate_swap_change(
            clmm_pubkey,
            params.pool_id,
            params.user_input_token,
            params.amount_specified,
            params.limit_price,
            base_in,
            params.slippage_bps,
            epoch,
            pool_state,
            rsps,
            tick_arrays,
        )?;
        Ok((result, tickarray_bitmap_extension))
    }

    pub async fn swap_clmm(
        &self,
        user_output_token: solana_pubkey::Pubkey,
        clmm_swap_change_result: ClmmSwapChangeResult,
        tick_array_bitmap_extension: solana_pubkey::Pubkey,
    ) -> anyhow::Result<Signature> {
        let mut instructions = Vec::new();
        let user_output_token = Pubkey::from(user_output_token.to_bytes());
        let mut remaining_accounts = Vec::new();
        remaining_accounts.push(AccountMeta::new_readonly(
            Address::from(tick_array_bitmap_extension.to_bytes()),
            false,
        ));
        let mut accounts = clmm_swap_change_result
            .remaining_tick_array_keys
            .into_iter()
            .map(|tick_array_address| {
                AccountMeta::new(Address::from(tick_array_address.to_bytes()), false)
            })
            .collect();
        remaining_accounts.append(&mut accounts);
        let user_output_token = solana_pubkey::Pubkey::from(user_output_token.to_bytes());
        let swap_instr = self.swap_v2_instr(
            clmm_swap_change_result.pool_amm_config,
            clmm_swap_change_result.pool_id,
            clmm_swap_change_result.input_vault,
            clmm_swap_change_result.output_vault,
            clmm_swap_change_result.pool_observation,
            clmm_swap_change_result.user_input_token,
            user_output_token,
            clmm_swap_change_result.input_vault_mint,
            clmm_swap_change_result.output_vault_mint,
            remaining_accounts,
            clmm_swap_change_result.amount,
            clmm_swap_change_result.other_amount_threshold,
            clmm_swap_change_result.sqrt_price_limit_x64,
            clmm_swap_change_result.is_base_input,
        )?;
        instructions.extend(swap_instr);

        self.send_and_sign_transaction(&instructions).await
    }

    pub fn swap_v2_instr(
        &self,
        amm_config: solana_pubkey::Pubkey,
        pool_account_key: solana_pubkey::Pubkey,
        input_vault: solana_pubkey::Pubkey,
        output_vault: solana_pubkey::Pubkey,
        observation_state: solana_pubkey::Pubkey,
        user_input_token: solana_pubkey::Pubkey,
        user_output_token: solana_pubkey::Pubkey,
        input_vault_mint: solana_pubkey::Pubkey,
        output_vault_mint: solana_pubkey::Pubkey,
        remaining_accounts: Vec<AccountMeta>,
        amount: u64,
        other_amount_threshold: u64,
        sqrt_price_limit_x64: Option<u128>,
        is_base_input: bool,
    ) -> anyhow::Result<Vec<Instruction>> {
        // Build Anchor-style instruction data for SwapV2:
        // discriminator + borsh-encoded fields.
        let mut data = Vec::with_capacity(8 + 8 + 8 + 16 + 1);
        data.extend_from_slice(&swap_v2_discriminator());
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(&other_amount_threshold.to_le_bytes());
        data.extend_from_slice(&sqrt_price_limit_x64.unwrap_or(0u128).to_le_bytes());
        data.push(is_base_input as u8);

        // Convert clmm Pubkey type into the Pubkey used by `AccountMeta`.
        fn to_sdk_pubkey(pk: solana_pubkey::Pubkey) -> Pubkey {
            Pubkey::from(pk.to_bytes())
        }

        let mut accounts: Vec<AccountMeta> = Vec::with_capacity(13 + remaining_accounts.len());

        // Core SwapSingleV2 accounts in the correct order.
        accounts.push(AccountMeta::new(self.owner.pubkey(), true)); // payer
        accounts.push(AccountMeta::new_readonly(to_sdk_pubkey(amm_config), false));
        accounts.push(AccountMeta::new(to_sdk_pubkey(pool_account_key), false));
        accounts.push(AccountMeta::new(to_sdk_pubkey(user_input_token), false));
        accounts.push(AccountMeta::new(to_sdk_pubkey(user_output_token), false));
        accounts.push(AccountMeta::new(to_sdk_pubkey(input_vault), false));
        accounts.push(AccountMeta::new(to_sdk_pubkey(output_vault), false));
        accounts.push(AccountMeta::new(to_sdk_pubkey(observation_state), false));
        accounts.push(AccountMeta::new_readonly(spl_token::id(), false));
        accounts.push(AccountMeta::new_readonly(
            Address::from(spl_token_2022::id().to_bytes()),
            false,
        ));
        accounts.push(AccountMeta::new_readonly(
            Address::from(spl_memo::id().to_bytes()),
            false,
        ));
        accounts.push(AccountMeta::new_readonly(
            to_sdk_pubkey(input_vault_mint),
            false,
        ));
        accounts.push(AccountMeta::new_readonly(
            to_sdk_pubkey(output_vault_mint),
            false,
        ));

        // Tick array bitmap extension and tick-array accounts.
        accounts.extend(remaining_accounts);

        let program_id = Pubkey::from_str_const(CLMM);

        Ok(vec![Instruction {
            program_id,
            accounts,
            data,
        }])
    }
}
