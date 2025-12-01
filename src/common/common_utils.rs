use crate::common::{TEN_THOUSAND, TransferFeeInfo};
use anchor_lang::AccountDeserialize;
use anyhow::{Result, anyhow, format_err};
use solana_address::Address;
use solana_program_pack::Pack as SolanaProgramPack;
use solana_sdk::{account::Account as CliAccount, pubkey::Pubkey, signer::keypair::Keypair};
use spl_token::solana_program::program_pack::Pack;
use spl_token_2022::{
    extension::{
        BaseState, BaseStateWithExtensions, StateWithExtensions,
        transfer_fee::{MAX_FEE_BASIS_POINTS, TransferFeeConfig},
    },
    state::{Account, Mint},
};
use std::convert::TryFrom;

pub fn amount_with_slippage(amount: u64, slippage_bps: u64, up_towards: bool) -> Result<u64> {
    let amount = amount as u128;
    let slippage_bps = slippage_bps as u128;
    let amount_with_slippage = if up_towards {
        amount
            .checked_mul(slippage_bps.checked_add(TEN_THOUSAND).unwrap())
            .unwrap()
            .checked_div(TEN_THOUSAND)
            .unwrap()
    } else {
        amount
            .checked_mul(TEN_THOUSAND.checked_sub(slippage_bps).unwrap())
            .unwrap()
            .checked_div(TEN_THOUSAND)
            .unwrap()
    };
    u64::try_from(amount_with_slippage)
        .map_err(|_| format_err!("failed to read keypair from {}", amount_with_slippage))
}

pub fn read_keypair_file(s: &str) -> Result<Keypair> {
    solana_sdk::signature::read_keypair_file(s)
        .map_err(|_| format_err!("failed to read keypair from {}", s))
}

pub enum TokenAccountState<'a> {
    SplToken(spl_token::state::Account),
    SplToken2022(StateWithExtensions<'a, Account>),
}
pub fn unpack_token<'a>(owner: &Address, token_data: &'a [u8]) -> Result<TokenAccountState<'a>> {
    if owner == &spl_token::id() {
        Ok(TokenAccountState::SplToken(unpack_spl(token_data)?))
    } else if solana_pubkey::Pubkey::from(owner.to_bytes()) == spl_token_2022::id() {
        Ok(TokenAccountState::SplToken2022(unpack_spl_2022(
            token_data,
        )?))
    } else {
        Err(anyhow!("owner is not spl program"))
    }
}

pub fn unpack_spl(token_data: &[u8]) -> Result<spl_token::state::Account> {
    // Avoid panics inside the SPL token library if we accidentally pass
    // mint data (82 bytes) or any other shorter buffer instead of a
    // 165‑byte token account.
    if token_data.len() < spl_token::state::Account::LEN {
        return Err(format_err!(
            "invalid spl-token account length: expected at least {}, got {}",
            spl_token::state::Account::LEN,
            token_data.len()
        ));
    }
    Ok(spl_token::state::Account::unpack_from_slice(token_data)?)
}

pub fn unpack_spl_2022(token_data: &[u8]) -> Result<StateWithExtensions<'_, Account>> {
    // Guard against accidentally passing a mint (82 bytes) or other
    // non‑token account data into the SPL 2022 `Account` unpacker,
    // which would otherwise panic inside the underlying library.
    if token_data.len() < Account::LEN {
        return Err(format_err!(
            "invalid spl-token-2022 token account length: expected at least {}, got {}",
            Account::LEN,
            token_data.len()
        ));
    }
    Ok(StateWithExtensions::<Account>::unpack(token_data)?)
}

pub fn unpack_mint(token_data: &[u8]) -> Result<StateWithExtensions<'_, Mint>> {
    let mint = StateWithExtensions::<Mint>::unpack(token_data)?;
    Ok(mint)
}

pub fn deserialize_anchor_account<T: AccountDeserialize>(account: &CliAccount) -> Result<T> {
    let mut data: &[u8] = &account.data;
    T::try_deserialize(&mut data).map_err(Into::into)
}

pub fn deserialize_account<T: Copy>(account: &CliAccount, is_anchor_account: bool) -> Result<T> {
    let mut account_data = account.data.as_slice();
    if is_anchor_account {
        account_data = &account_data[8..size_of::<T>() + 8];
    }
    Ok(unsafe { *(&account_data[0] as *const u8 as *const T) })
}

pub async fn get_pool_mints_inverse_fee(
    rpc_client: &solana_client::nonblocking::rpc_client::RpcClient,
    token_mint_0: Pubkey,
    token_mint_1: Pubkey,
    post_fee_amount_0: u64,
    post_fee_amount_1: u64,
) -> Result<(TransferFeeInfo, TransferFeeInfo)> {
    let load_accounts = vec![token_mint_0, token_mint_1];
    let rsps = rpc_client.get_multiple_accounts(&load_accounts).await?;
    let epoch = rpc_client.get_epoch_info().await?.epoch;
    // todo fix
    let mint0_account = rsps[0].clone().ok_or(anyhow!("load mint0 rps error!"))?;
    let mint1_account = rsps[1].clone().ok_or(anyhow!("load mint0 rps error!"))?;
    let mint0_state = unpack_mint(&mint0_account.data)?;
    let mint1_state = unpack_mint(&mint1_account.data)?;
    Ok((
        TransferFeeInfo {
            mint: token_mint_0,
            owner: mint0_account.owner,
            transfer_fee: get_transfer_inverse_fee(&mint0_state, post_fee_amount_0, epoch)?,
        },
        TransferFeeInfo {
            mint: token_mint_1,
            owner: mint1_account.owner,
            transfer_fee: get_transfer_inverse_fee(&mint1_state, post_fee_amount_1, epoch)?,
        },
    ))
}

/// Calculate the fee for output amount
pub fn get_transfer_inverse_fee<S: BaseState + SolanaProgramPack>(
    account_state: &StateWithExtensions<S>,
    epoch: u64,
    post_fee_amount: u64,
) -> Result<u64> {
    if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            Ok(u64::from(transfer_fee.maximum_fee))
        } else {
            Ok(transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .ok_or(anyhow!("calculate_inverse_epoch_fee returned None"))?)
        }
    } else {
        Ok(0)
    }
}

/// Calculate the fee for input amount
pub fn get_transfer_fee<S: BaseState + SolanaProgramPack>(
    account_state: &StateWithExtensions<S>,
    epoch: u64,
    pre_fee_amount: u64,
) -> u64 {
    if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(epoch, pre_fee_amount)
            .unwrap()
    } else {
        0
    }
}
