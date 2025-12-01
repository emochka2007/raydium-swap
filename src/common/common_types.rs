use solana_sdk::pubkey::Pubkey;
use spl_token_2022::extension::{
    confidential_transfer::{ConfidentialTransferAccount, ConfidentialTransferMint},
    cpi_guard::CpiGuard,
    default_account_state::DefaultAccountState,
    immutable_owner::ImmutableOwner,
    interest_bearing_mint::InterestBearingConfig,
    memo_transfer::MemoTransfer,
    mint_close_authority::MintCloseAuthority,
    non_transferable::{NonTransferable, NonTransferableAccount},
    permanent_delegate::PermanentDelegate,
    transfer_fee::{TransferFeeAmount, TransferFeeConfig},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenInfo {
    pub key: Pubkey,
    pub mint: Pubkey,
    pub program: Pubkey,
    pub amount: u64,
    pub decimals: u8,
}

#[derive(Debug)]
pub enum ExtensionStruct {
    ConfidentialTransferAccount(ConfidentialTransferAccount),
    ConfidentialTransferMint(ConfidentialTransferMint),
    CpiGuard(CpiGuard),
    DefaultAccountState(DefaultAccountState),
    ImmutableOwner(ImmutableOwner),
    InterestBearingConfig(InterestBearingConfig),
    MemoTransfer(MemoTransfer),
    MintCloseAuthority(MintCloseAuthority),
    NonTransferable(NonTransferable),
    NonTransferableAccount(NonTransferableAccount),
    PermanentDelegate(PermanentDelegate),
    TransferFeeConfig(TransferFeeConfig),
    TransferFeeAmount(TransferFeeAmount),
}

pub const TEN_THOUSAND: u128 = 10000;
#[derive(Debug)]
pub struct TransferFeeInfo {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub transfer_fee: u64,
}

pub enum InstructionDecodeType {
    BaseHex,
    Base64,
    Base58,
}
pub const PROGRAM_LOG: &str = "Program log: ";
pub const PROGRAM_DATA: &str = "Program data: ";
pub const RAY_LOG: &str = "ray_log: ";
