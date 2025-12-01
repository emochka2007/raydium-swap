use crate::common::{TEN_THOUSAND, TransferFeeInfo};
use anchor_lang::AccountDeserialize;
use anyhow::{Result, format_err};
use solana_address::Address;
use solana_client::rpc_client::RpcClient;
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
        unreachable!()
    }
    // Try to unpack legacy SPL token account data first, then fall back to SPL 2022.
    // match unpack_spl(token_data) {
    //     Ok(info) => Ok(TokenAccountState::SplToken(info)),
    //     Err(e) => {
    //         warn!("Error decoding legacy, trying spl2022 {:?}", e);
    //         Ok(TokenAccountState::SplToken2022(unpack_spl_2022(
    //             token_data,
    //         )?))
    //     }
    // }
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
    if token_data.len() < spl_token_2022::state::Account::LEN {
        return Err(format_err!(
            "invalid spl-token-2022 token account length: expected at least {}, got {}",
            spl_token_2022::state::Account::LEN,
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
        account_data = &account_data[8..std::mem::size_of::<T>() + 8];
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
    let mint0_account = rsps[0].clone().ok_or("load mint0 rps error!").unwrap();
    let mint1_account = rsps[1].clone().ok_or("load mint0 rps error!").unwrap();
    let mint0_state = unpack_mint(&mint0_account.data)?;
    let mint1_state = unpack_mint(&mint1_account.data)?;
    Ok((
        TransferFeeInfo {
            mint: token_mint_0,
            owner: mint0_account.owner,
            transfer_fee: get_transfer_inverse_fee(&mint0_state, post_fee_amount_0, epoch),
        },
        TransferFeeInfo {
            mint: token_mint_1,
            owner: mint1_account.owner,
            transfer_fee: get_transfer_inverse_fee(&mint1_state, post_fee_amount_1, epoch),
        },
    ))
}

pub fn get_pool_mints_transfer_fee(
    rpc_client: &RpcClient,
    token_mint_0: Pubkey,
    token_mint_1: Pubkey,
    pre_fee_amount_0: u64,
    pre_fee_amount_1: u64,
) -> (TransferFeeInfo, TransferFeeInfo) {
    let load_accounts = vec![token_mint_0, token_mint_1];
    let rsps = rpc_client.get_multiple_accounts(&load_accounts).unwrap();
    let epoch = rpc_client.get_epoch_info().unwrap().epoch;
    let mint0_account = rsps[0].clone().ok_or("load mint0 rps error!").unwrap();
    let mint1_account = rsps[1].clone().ok_or("load mint0 rps error!").unwrap();
    let mint0_state = unpack_mint(&mint0_account.data).unwrap();
    let mint1_state = unpack_mint(&mint1_account.data).unwrap();
    (
        TransferFeeInfo {
            mint: token_mint_0,
            owner: mint0_account.owner,
            transfer_fee: get_transfer_fee(&mint0_state, epoch, pre_fee_amount_0),
        },
        TransferFeeInfo {
            mint: token_mint_1,
            owner: mint1_account.owner,
            transfer_fee: get_transfer_fee(&mint1_state, epoch, pre_fee_amount_1),
        },
    )
}

/// Calculate the fee for output amount
pub fn get_transfer_inverse_fee<S: BaseState + SolanaProgramPack>(
    account_state: &StateWithExtensions<S>,
    epoch: u64,
    post_fee_amount: u64,
) -> u64 {
    
    if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .unwrap()
        }
    } else {
        0
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

// pub fn get_nft_accounts_by_owner_with_specified_program(
//     client: &RpcClient,
//     owner: &Pubkey,
//     token_program: Pubkey,
// ) -> Vec<TokenInfo> {
//     let all_tokens = client
//         .get_token_accounts_by_owner(owner, TokenAccountsFilter::ProgramId(spl_token::id()))
//         .unwrap();
//     let mut nft_accounts_info = Vec::new();
//     for keyed_account in all_tokens {
//         if let UiAccountData::Json(parsed_account) = keyed_account.account.data {
//             if parsed_account.program == "spl-token" || parsed_account.program == "spl-token-2022" {
//                 if let Ok(TokenAccountType::Account(ui_token_account)) =
//                     serde_json::from_value(parsed_account.parsed)
//                 {
//                     let _frozen = ui_token_account.state == UiAccountState::Frozen;
//
//                     let token = ui_token_account
//                         .mint
//                         .parse::<Pubkey>()
//                         .unwrap_or_else(|err| panic!("Invalid mint: {}", err));
//                     let token_account = keyed_account
//                         .pubkey
//                         .parse::<Pubkey>()
//                         .unwrap_or_else(|err| panic!("Invalid token account: {}", err));
//                     let token_amount = ui_token_account
//                         .token_amount
//                         .amount
//                         .parse::<u64>()
//                         .unwrap_or_else(|err| panic!("Invalid token amount: {}", err));
//
//                     let _close_authority = ui_token_account.close_authority.map_or(*owner, |s| {
//                         s.parse::<Pubkey>()
//                             .unwrap_or_else(|err| panic!("Invalid close authority: {}", err))
//                     });
//
//                     if ui_token_account.token_amount.decimals == 0 && token_amount == 1 {
//                         nft_accounts_info.push(TokenInfo {
//                             key: token_account,
//                             mint: token,
//                             program: token_program,
//                             amount: token_amount,
//                             decimals: ui_token_account.token_amount.decimals,
//                         });
//                     }
//                 }
//             }
//         }
//     }
//     nft_accounts_info
// }
//
// pub fn get_account_extensions<'data, S: BaseState>(
//     account_state: &StateWithExtensions<'data, S>,
// ) -> Vec<ExtensionStruct> {
//     let mut extensions: Vec<ExtensionStruct> = Vec::new();
//     let extension_types = account_state.get_extension_types().unwrap();
//     println!("extension_types:{:?}", extension_types);
//     for extension_type in extension_types {
//         match extension_type {
//             ExtensionType::ConfidentialTransferAccount => {
//                 let extension = account_state
//                     .get_extension::<ConfidentialTransferAccount>()
//                     .unwrap();
//                 extensions.push(ExtensionStruct::ConfidentialTransferAccount(*extension));
//             }
//             ExtensionType::ConfidentialTransferMint => {
//                 let extension = account_state
//                     .get_extension::<ConfidentialTransferMint>()
//                     .unwrap();
//                 extensions.push(ExtensionStruct::ConfidentialTransferMint(*extension));
//             }
//             ExtensionType::CpiGuard => {
//                 let extension = account_state.get_extension::<CpiGuard>().unwrap();
//                 extensions.push(ExtensionStruct::CpiGuard(*extension));
//             }
//             ExtensionType::DefaultAccountState => {
//                 let extension = account_state
//                     .get_extension::<DefaultAccountState>()
//                     .unwrap();
//                 extensions.push(ExtensionStruct::DefaultAccountState(*extension));
//             }
//             ExtensionType::ImmutableOwner => {
//                 let extension = account_state.get_extension::<ImmutableOwner>().unwrap();
//                 extensions.push(ExtensionStruct::ImmutableOwner(*extension));
//             }
//             ExtensionType::InterestBearingConfig => {
//                 let extension = account_state
//                     .get_extension::<InterestBearingConfig>()
//                     .unwrap();
//                 extensions.push(ExtensionStruct::InterestBearingConfig(*extension));
//             }
//             ExtensionType::MemoTransfer => {
//                 let extension = account_state.get_extension::<MemoTransfer>().unwrap();
//                 extensions.push(ExtensionStruct::MemoTransfer(*extension));
//             }
//             ExtensionType::MintCloseAuthority => {
//                 let extension = account_state.get_extension::<MintCloseAuthority>().unwrap();
//                 extensions.push(ExtensionStruct::MintCloseAuthority(*extension));
//             }
//             ExtensionType::NonTransferable => {
//                 let extension = account_state.get_extension::<NonTransferable>().unwrap();
//                 extensions.push(ExtensionStruct::NonTransferable(*extension));
//             }
//             ExtensionType::NonTransferableAccount => {
//                 let extension = account_state
//                     .get_extension::<NonTransferableAccount>()
//                     .unwrap();
//                 extensions.push(ExtensionStruct::NonTransferableAccount(*extension));
//             }
//             ExtensionType::PermanentDelegate => {
//                 let extension = account_state.get_extension::<PermanentDelegate>().unwrap();
//                 extensions.push(ExtensionStruct::PermanentDelegate(*extension));
//             }
//             ExtensionType::TransferFeeConfig => {
//                 let extension = account_state.get_extension::<TransferFeeConfig>().unwrap();
//                 extensions.push(ExtensionStruct::TransferFeeConfig(*extension));
//             }
//             ExtensionType::TransferFeeAmount => {
//                 let extension = account_state.get_extension::<TransferFeeAmount>().unwrap();
//                 extensions.push(ExtensionStruct::TransferFeeAmount(*extension));
//             }
//             _ => {
//                 println!("unkonwn extension:{:#?}", extension_type);
//             }
//         }
//     }
//     extensions
// }
