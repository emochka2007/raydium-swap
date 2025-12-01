use crate::clmm::{
    ClmmSwapChangeResult,
    StepComputations, SwapState, price_to_sqrt_price_x64,
};
use crate::common::{
    TokenAccountState, amount_with_slippage, common_utils, deserialize_anchor_account, get_transfer_fee, rpc, unpack_mint, unpack_token,
};
use crate::libraries::{
    MAX_SQRT_PRICE_X64, MAX_TICK, MIN_SQRT_PRICE_X64, MIN_TICK, add_delta, compute_swap_step, get_sqrt_price_at_tick, get_tick_at_sqrt_price,
};
use crate::states::{
    AmmConfig, PoolState, TICK_ARRAY_SEED, TickArrayBitmapExtension, TickArrayState, TickState,
};
use anchor_lang::solana_program::program_option::COption as AnchorCOption;
use anyhow::Result;
use arrayref::array_ref;
use solana_address::Address;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use spl_token_2022::state::AccountState;
use std::{
    collections::VecDeque,
    ops::{DerefMut, Neg},
};

pub async fn calculate_swap_change(
    rpc_client: &RpcClient,
    raydium_v3_program: Pubkey,
    pool_id: Pubkey,
    tickarray_bitmap_extension: Pubkey,
    input_token: Pubkey,
    amount: u64,
    limit_price: Option<f64>,
    base_in: bool,
    slippage_bps: u64,
) -> Result<ClmmSwapChangeResult> {
    let pool_id = solana_address::Address::from(pool_id.to_bytes());
    let pool_state = rpc::get_anchor_account::<PoolState>(rpc_client, &pool_id)
        .await?
        .unwrap();
    // load mult account
    let load_accounts: Vec<Address> = [input_token,
        pool_state.amm_config,
        pool_state.token_mint_0,
        pool_state.token_mint_1,
        tickarray_bitmap_extension]
    .iter()
    .map(|pubkey| Address::from(pubkey.to_bytes()))
    .collect();

    let rsps = rpc_client.get_multiple_accounts(&load_accounts).await?;
    let epoch = rpc_client.get_epoch_info().await?.epoch;
    let [
        user_input_account,
        amm_config_account,
        mint0_account,
        mint1_account,
        tickarray_bitmap_extension_account,
    ] = array_ref![rsps, 0, 5];
    let mint0_token_program = mint0_account.as_ref().unwrap().owner;
    let mint1_token_program = mint1_account.as_ref().unwrap().owner;
    let input_mint_owner = user_input_account.as_ref().unwrap().owner;
    let user_input_state = match unpack_token(
        &input_mint_owner,
        &user_input_account.as_ref().unwrap().data,
    )? {
        TokenAccountState::SplToken(info) => {
            // Convert legacy SPL token account representation into the SPL 2022
            // `Account` type, carefully translating between different `COption`
            // definitions from separate Solana crates.
            use spl_token::solana_program::program_option::COption as SplCOption;

            spl_token_2022::state::Account {
                mint: solana_pubkey::Pubkey::from(info.mint.to_bytes()),
                owner: solana_pubkey::Pubkey::from(info.owner.to_bytes()),
                amount,
                delegate: AnchorCOption::None,
                state: AccountState::Initialized,
                is_native: match info.is_native {
                    SplCOption::Some(v) => AnchorCOption::Some(v),
                    SplCOption::None => AnchorCOption::None,
                },
                delegated_amount: info.delegated_amount,
                close_authority: AnchorCOption::None,
            }
        }
        TokenAccountState::SplToken2022(state) => state.base,
    };
    // let user_input_state: Account = unpack_spl_2022(&user_input_account.as_ref().unwrap().data)?;
    let mint0_state = unpack_mint(&mint0_account.as_ref().unwrap().data)?;
    let mint1_state = unpack_mint(&mint1_account.as_ref().unwrap().data)?;
    let tickarray_bitmap_extension_state = deserialize_anchor_account::<TickArrayBitmapExtension>(
        tickarray_bitmap_extension_account.as_ref().unwrap(),
    )?;
    let amm_config_state =
        deserialize_anchor_account::<AmmConfig>(amm_config_account.as_ref().unwrap())?;

    let (
        zero_for_one,
        input_vault,
        output_vault,
        input_vault_mint,
        output_vault_mint,
        input_token_program,
        output_token_program,
    ) = if user_input_state.mint == pool_state.token_mint_0 {
        (
            true,
            pool_state.token_vault_0,
            pool_state.token_vault_1,
            pool_state.token_mint_0,
            pool_state.token_mint_1,
            mint0_token_program,
            mint1_token_program,
        )
    } else if user_input_state.mint == pool_state.token_mint_1 {
        (
            false,
            pool_state.token_vault_1,
            pool_state.token_vault_0,
            pool_state.token_mint_1,
            pool_state.token_mint_0,
            mint1_token_program,
            mint0_token_program,
        )
    } else {
        panic!("input tokens not match pool vaults");
    };
    let transfer_fee = if base_in {
        if zero_for_one {
            get_transfer_fee(&mint0_state, epoch, amount)
        } else {
            get_transfer_fee(&mint1_state, epoch, amount)
        }
    } else {
        0
    };
    let amount_specified = amount.checked_sub(transfer_fee).unwrap();
    // load tick_arrays
    let mut tick_arrays = load_cur_and_next_five_tick_array(
        rpc_client,
        raydium_v3_program,
        Pubkey::from(pool_id.to_bytes()),
        &pool_state,
        &tickarray_bitmap_extension_state,
        zero_for_one,
    )
    .await?;
    let sqrt_price_limit_x64 = if limit_price.is_some() {
        let sqrt_price_x64 = price_to_sqrt_price_x64(
            limit_price.unwrap(),
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );
        Some(sqrt_price_x64)
    } else {
        None
    };

    let (mut other_amount_threshold, tick_array_indexes) =
        get_out_put_amount_and_remaining_accounts(
            amount_specified,
            sqrt_price_limit_x64,
            zero_for_one,
            base_in,
            amm_config_state.trade_fee_rate,
            &pool_state,
            &tickarray_bitmap_extension_state,
            &mut tick_arrays,
        )
        .unwrap();
    println!(
        "amount:{}, other_amount_threshold:{}",
        amount, other_amount_threshold
    );
    let remaining_tick_array_keys = tick_array_indexes
        .into_iter()
        .map(|index| {
            Pubkey::find_program_address(
                &[
                    TICK_ARRAY_SEED.as_bytes(),
                    pool_id.to_bytes().as_ref(),
                    &index.to_be_bytes(),
                ],
                &raydium_v3_program,
            )
            .0
        })
        .collect();
    if base_in {
        // calc mint out amount with slippage
        other_amount_threshold = amount_with_slippage(other_amount_threshold, slippage_bps, false)?;
    } else {
        // calc max in with slippage
        other_amount_threshold = amount_with_slippage(other_amount_threshold, slippage_bps, true)?;
        // calc max in with transfer_fee
        let transfer_fee = if zero_for_one {
            common_utils::get_transfer_inverse_fee(&mint0_state, epoch, other_amount_threshold)
        } else {
            common_utils::get_transfer_inverse_fee(&mint1_state, epoch, other_amount_threshold)
        };
        other_amount_threshold += transfer_fee;
    }
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

async fn load_cur_and_next_five_tick_array(
    rpc_client: &RpcClient,
    raydium_v3_program: Pubkey,
    pool_id: Pubkey,
    pool_state: &PoolState,
    tickarray_bitmap_extension: &TickArrayBitmapExtension,
    zero_for_one: bool,
) -> Result<VecDeque<TickArrayState>> {
    let (_, mut current_valid_tick_array_start_index) = pool_state
        .get_first_initialized_tick_array(&Some(*tickarray_bitmap_extension), zero_for_one)?;
    let mut tick_array_keys = Vec::new();
    tick_array_keys.push(
        Pubkey::find_program_address(
            &[
                TICK_ARRAY_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                &current_valid_tick_array_start_index.to_be_bytes(),
            ],
            &raydium_v3_program,
        )
        .0,
    );
    let mut max_array_size = 5;
    while max_array_size != 0 {
        let next_tick_array_index = pool_state.next_initialized_tick_array_start_index(
            &Some(*tickarray_bitmap_extension),
            current_valid_tick_array_start_index,
            zero_for_one,
        )?;
        if next_tick_array_index.is_none() {
            break;
        }
        current_valid_tick_array_start_index = next_tick_array_index.unwrap();
        tick_array_keys.push(
            Pubkey::find_program_address(
                &[
                    TICK_ARRAY_SEED.as_bytes(),
                    pool_id.to_bytes().as_ref(),
                    &current_valid_tick_array_start_index.to_be_bytes(),
                ],
                &raydium_v3_program,
            )
            .0,
        );
        max_array_size -= 1;
    }
    let tick_array_keys: Vec<Address> = tick_array_keys
        .iter()
        .map(|addr| Address::from(addr.to_bytes()))
        .collect();
    let tick_array_rsps = rpc_client.get_multiple_accounts(&tick_array_keys).await?;
    let mut tick_arrays = VecDeque::new();
    for tick_array in tick_array_rsps {
        let tick_array_state = deserialize_anchor_account::<TickArrayState>(&tick_array.unwrap())?;
        tick_arrays.push_back(tick_array_state);
    }
    Ok(tick_arrays)
}

pub fn get_out_put_amount_and_remaining_accounts(
    input_amount: u64,
    sqrt_price_limit_x64: Option<u128>,
    zero_for_one: bool,
    is_base_input: bool,
    trade_fee_rate: u32,
    pool_state: &PoolState,
    tickarray_bitmap_extension: &TickArrayBitmapExtension,
    tick_arrays: &mut VecDeque<TickArrayState>,
) -> Result<(u64, VecDeque<i32>), &'static str> {
    let (is_pool_current_tick_array, current_valid_tick_array_start_index) = pool_state
        .get_first_initialized_tick_array(&Some(*tickarray_bitmap_extension), zero_for_one)
        .unwrap();

    let (amount_calculated, tick_array_start_index_vec) = swap_compute(
        zero_for_one,
        is_base_input,
        is_pool_current_tick_array,
        trade_fee_rate,
        input_amount,
        current_valid_tick_array_start_index,
        sqrt_price_limit_x64.unwrap_or(0),
        pool_state,
        tickarray_bitmap_extension,
        tick_arrays,
    )?;
    println!("tick_array_start_index:{:?}", tick_array_start_index_vec);

    Ok((amount_calculated, tick_array_start_index_vec))
}

fn swap_compute(
    zero_for_one: bool,
    is_base_input: bool,
    is_pool_current_tick_array: bool,
    trade_fee_rate: u32,
    amount_specified: u64,
    current_valid_tick_array_start_index: i32,
    sqrt_price_limit_x64: u128,
    pool_state: &PoolState,
    tickarray_bitmap_extension: &TickArrayBitmapExtension,
    tick_arrays: &mut VecDeque<TickArrayState>,
) -> Result<(u64, VecDeque<i32>), &'static str> {
    if amount_specified == 0 {
        return Err("amountSpecified must not be 0");
    }
    let sqrt_price_limit_x64 = if sqrt_price_limit_x64 == 0 {
        if zero_for_one {
            MIN_SQRT_PRICE_X64 + 1
        } else {
            MAX_SQRT_PRICE_X64 - 1
        }
    } else {
        sqrt_price_limit_x64
    };
    if zero_for_one {
        if sqrt_price_limit_x64 < MIN_SQRT_PRICE_X64 {
            return Err("sqrt_price_limit_x64 must greater than MIN_SQRT_PRICE_X64");
        }
        if sqrt_price_limit_x64 >= pool_state.sqrt_price_x64 {
            return Err("sqrt_price_limit_x64 must smaller than current");
        }
    } else {
        if sqrt_price_limit_x64 > MAX_SQRT_PRICE_X64 {
            return Err("sqrt_price_limit_x64 must smaller than MAX_SQRT_PRICE_X64");
        }
        if sqrt_price_limit_x64 <= pool_state.sqrt_price_x64 {
            return Err("sqrt_price_limit_x64 must greater than current");
        }
    }
    let mut tick_match_current_tick_array = is_pool_current_tick_array;

    let mut state = SwapState {
        amount_specified_remaining: amount_specified,
        amount_calculated: 0,
        sqrt_price_x64: pool_state.sqrt_price_x64,
        tick: pool_state.tick_current,
        liquidity: pool_state.liquidity,
    };

    let mut tick_array_current = tick_arrays.pop_front().unwrap();
    if tick_array_current.start_tick_index != current_valid_tick_array_start_index {
        return Err("tick array start tick index does not match");
    }
    let mut tick_array_start_index_vec = VecDeque::new();
    tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);
    let mut loop_count = 0;
    // loop across ticks until input liquidity is consumed, or the limit price is reached
    while state.amount_specified_remaining != 0
        && state.sqrt_price_x64 != sqrt_price_limit_x64
        && state.tick < MAX_TICK
        && state.tick > MIN_TICK
    {
        if loop_count > 10 {
            return Err("loop_count limit");
        }
        let mut step = StepComputations::default();
        step.sqrt_price_start_x64 = state.sqrt_price_x64;
        // save the bitmap, and the tick account if it is initialized
        let mut next_initialized_tick = if let Some(tick_state) = tick_array_current
            .next_initialized_tick(state.tick, pool_state.tick_spacing, zero_for_one)
            .unwrap()
        {
            Box::new(*tick_state)
        } else if !tick_match_current_tick_array {
            tick_match_current_tick_array = true;
            Box::new(
                *tick_array_current
                    .first_initialized_tick(zero_for_one)
                    .unwrap(),
            )
        } else {
            Box::new(TickState::default())
        };
        if !next_initialized_tick.is_initialized() {
            let current_vaild_tick_array_start_index = pool_state
                .next_initialized_tick_array_start_index(
                    &Some(*tickarray_bitmap_extension),
                    current_valid_tick_array_start_index,
                    zero_for_one,
                )
                .unwrap();
            tick_array_current = tick_arrays.pop_front().unwrap();
            if current_vaild_tick_array_start_index.is_none() {
                return Err("tick array start tick index out of range limit");
            }
            if tick_array_current.start_tick_index != current_vaild_tick_array_start_index.unwrap()
            {
                return Err("tick array start tick index does not match");
            }
            tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);
            let mut first_initialized_tick = tick_array_current
                .first_initialized_tick(zero_for_one)
                .unwrap();

            next_initialized_tick = Box::new(*first_initialized_tick.deref_mut());
        }
        step.tick_next = next_initialized_tick.tick;
        step.initialized = next_initialized_tick.is_initialized();
        if step.tick_next < MIN_TICK {
            step.tick_next = MIN_TICK;
        } else if step.tick_next > MAX_TICK {
            step.tick_next = MAX_TICK;
        }

        step.sqrt_price_next_x64 = get_sqrt_price_at_tick(step.tick_next).unwrap();

        let target_price = if (zero_for_one && step.sqrt_price_next_x64 < sqrt_price_limit_x64)
            || (!zero_for_one && step.sqrt_price_next_x64 > sqrt_price_limit_x64)
        {
            sqrt_price_limit_x64
        } else {
            step.sqrt_price_next_x64
        };
        let swap_step = compute_swap_step(
            state.sqrt_price_x64,
            target_price,
            state.liquidity,
            state.amount_specified_remaining,
            trade_fee_rate,
            is_base_input,
            zero_for_one,
            1,
        )
        .unwrap();
        state.sqrt_price_x64 = swap_step.sqrt_price_next_x64;
        step.amount_in = swap_step.amount_in;
        step.amount_out = swap_step.amount_out;
        step.fee_amount = swap_step.fee_amount;

        if is_base_input {
            state.amount_specified_remaining = state
                .amount_specified_remaining
                .checked_sub(step.amount_in + step.fee_amount)
                .unwrap();
            state.amount_calculated = state
                .amount_calculated
                .checked_add(step.amount_out)
                .unwrap();
        } else {
            state.amount_specified_remaining = state
                .amount_specified_remaining
                .checked_sub(step.amount_out)
                .unwrap();
            state.amount_calculated = state
                .amount_calculated
                .checked_add(step.amount_in + step.fee_amount)
                .unwrap();
        }

        if state.sqrt_price_x64 == step.sqrt_price_next_x64 {
            // if the tick is initialized, run the tick transition
            if step.initialized {
                let mut liquidity_net = next_initialized_tick.liquidity_net;
                if zero_for_one {
                    liquidity_net = liquidity_net.neg();
                }
                state.liquidity = add_delta(state.liquidity, liquidity_net).unwrap();
            }

            state.tick = if zero_for_one {
                step.tick_next - 1
            } else {
                step.tick_next
            };
        } else if state.sqrt_price_x64 != step.sqrt_price_start_x64 {
            // recompute unless we're on a lower tick boundary (i.e. already transitioned ticks), and haven't moved
            state.tick = get_tick_at_sqrt_price(state.sqrt_price_x64).unwrap();
        }
        loop_count += 1;
    }

    Ok((state.amount_calculated, tick_array_start_index_vec))
}
