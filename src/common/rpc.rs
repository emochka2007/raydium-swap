use anchor_lang::AccountDeserialize;
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

pub async fn get_anchor_account<T: AccountDeserialize>(
    client: &RpcClient,
    addr: &Pubkey,
) -> Result<Option<T>> {
    if let Some(account) = client
        .get_account_with_commitment(addr, CommitmentConfig::processed())
        .await?
        .value
    {
        let mut data: &[u8] = &account.data;
        let ret = T::try_deserialize(&mut data).unwrap();
        Ok(Some(ret))
    } else {
        Ok(None)
    }
}
