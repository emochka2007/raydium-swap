use crate::clmm::ClmmSwapChangeResult;
use crate::common::unpack_mint;
use crate::interface::{CalculateSwapChangeParams, Rsps, TickArrays};
use crate::states::PoolState;
use anyhow::anyhow;
use solana_pubkey::Pubkey;

pub fn calculate_swap_change(
    raydium_v3_program: Pubkey,
    pool_id: Pubkey,
    input_token: Pubkey,
    amount: u64,
    limit_price: Option<f64>,
    base_in: bool,
    slippage_bps: u64,
    epoch: u64,
    pool_state: PoolState,
    rsps: Rsps,
    tick_arrays: TickArrays,
) -> anyhow::Result<ClmmSwapChangeResult> {
    let pool_id = solana_address::Address::from(pool_id.to_bytes());

    let CalculateSwapChangeParams {
        mint0_account,
        mint1_account,
        tickarray_bitmap_extension_state,
        zero_for_one,
        amount_specified,
        amm_config_state,
        input_vault,
        output_vault,
        input_vault_mint,
        output_vault_mint,
        input_token_program,
        output_token_program,
        ..
    } = crate::clmm::clmm_utils::calculate_swap_change_accounts(
        &rsps, amount, pool_state, base_in, epoch,
    )?;

    let mint0_state = unpack_mint(
        &mint0_account
            .as_ref()
            .ok_or(anyhow!("Mint token program is None"))?
            .data,
    )?;
    let mint1_state = unpack_mint(
        &mint1_account
            .as_ref()
            .ok_or(anyhow!("Mint token program is None"))?
            .data,
    )?;
    let (remaining_tick_array_keys, other_amount_threshold, sqrt_price_limit_x64) =
        crate::clmm::clmm_utils::calculate_other_amount_threshold(
            pool_id,
            raydium_v3_program,
            slippage_bps,
            pool_state,
            tickarray_bitmap_extension_state,
            zero_for_one,
            amount_specified,
            amm_config_state,
            limit_price,
            base_in,
            tick_arrays,
            &mint0_state,
            &mint1_state,
            epoch,
        )?;

    Ok(ClmmSwapChangeResult {
        pool_amm_config: pool_state.amm_config,
        pool_id: Pubkey::from(pool_id.to_bytes()),
        pool_observation: pool_state.observation_key,
        input_vault,
        output_vault,
        input_vault_mint,
        output_vault_mint,
        input_token_program: Pubkey::from(input_token_program.to_bytes()),
        output_token_program: Pubkey::from(output_token_program.to_bytes()),
        user_input_token: input_token,
        remaining_tick_array_keys,
        amount,
        other_amount_threshold,
        sqrt_price_limit_x64,
        is_base_input: base_in,
    })
}
