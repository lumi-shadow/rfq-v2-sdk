
use anchor_lang::AnchorDeserialize;
use solana_sdk::pubkey::Pubkey;

use crate::error::FillDecoderError;
use crate::types::{FillAccounts, FillExactInArgs, FillExactInInstruction, FillExactInParams};

pub use crate::rfq_v2::{
    fill_exact_in_discriminator, is_fill_exact_in, FILL_ACCOUNT_LABELS,
};

pub const RFQ_V2_PROGRAM_ID: &str = "fd3nMFYTQjX1yr5ER8u7tPdHJB7qt8RpDpNtLQX2Br5";

pub fn decode_fill_instruction(data: &[u8]) -> crate::Result<FillExactInInstruction> {
    if !is_fill_exact_in(data) {
        return Err(FillDecoderError::validation(
            "instruction data does not match fill_exact_in discriminator",
        ));
    }

    let mut payload = &data[8..];
    let args = FillExactInArgs::deserialize(&mut payload).map_err(|e| {
        FillDecoderError::other(format!("failed to deserialize fill_exact_in args: {e}"))
    })?;

    Ok(args.into())
}

pub fn decode_fill_params(data: &[u8]) -> crate::Result<FillExactInParams> {
    let mut payload = data;
    FillExactInParams::deserialize(&mut payload).map_err(|e| {
        FillDecoderError::other(format!("failed to deserialize fill params: {e}"))
    })
}

pub fn decode_fill_args(data: &[u8]) -> crate::Result<FillExactInInstruction> {
    let mut payload = data;
    let args = FillExactInArgs::deserialize(&mut payload).map_err(|e| {
        FillDecoderError::other(format!("failed to deserialize fill args: {e}"))
    })?;
    Ok(args.into())
}

pub fn decode_fill_accounts(account_keys: &[Pubkey]) -> crate::Result<FillAccounts> {
    let expected = FILL_ACCOUNT_LABELS.len();
    if account_keys.len() < expected {
        return Err(FillDecoderError::validation(format!(
            "expected at least {} account keys, got {}",
            expected,
            account_keys.len()
        )));
    }

    Ok(FillAccounts {
        user: account_keys[0],
        fill_authority: account_keys[1],
        user_base_token_account: account_keys[2],
        user_quote_token_account: account_keys[3],
        maker_base_token_account: account_keys[4],
        maker_quote_token_account: account_keys[5],
        base_mint: account_keys[6],
        quote_mint: account_keys[7],
        base_token_program: account_keys[8],
        quote_token_program: account_keys[9],
        instructions_sysvar: account_keys[10],
    })
}

pub fn decode_fill_accounts_bytes(account_keys: &[[u8; 32]]) -> crate::Result<FillAccounts> {
    let pubkeys: Vec<Pubkey> = account_keys.iter().map(|k| Pubkey::new_from_array(*k)).collect();
    decode_fill_accounts(&pubkeys)
}
