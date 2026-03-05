use anchor_lang::{declare_program, Discriminator};

declare_program!(rfq_v2);

pub use rfq_v2::client::args::FillExactIn as FillExactInArgs;
pub use rfq_v2::types::*;
pub use rfq_v2::ID as RFQ_V2_PROGRAM_ID;

pub const FILL_ACCOUNT_LABELS: [&str; 11] = [
    "user",
    "fill_authority",
    "user_base_token_account",
    "user_quote_token_account",
    "maker_base_token_account",
    "maker_quote_token_account",
    "base_mint",
    "quote_mint",
    "base_token_program",
    "quote_token_program",
    "instructions_sysvar",
];

pub fn fill_exact_in_discriminator() -> &'static [u8] {
    <FillExactInArgs as Discriminator>::DISCRIMINATOR
}

pub fn is_fill_exact_in(instruction_data: &[u8]) -> bool {
    instruction_data.starts_with(fill_exact_in_discriminator())
}
