
use solana_sdk::pubkey::Pubkey;

pub use crate::rfq_v2::{FillExactInArgs, FillExactInParams, Level, Side};

#[derive(Debug, Clone)]
pub struct FillExactInInstruction {
    pub taker_side: Side,
    pub amount_in_atoms: u64,
    pub params: FillExactInParams,
}

impl From<FillExactInArgs> for FillExactInInstruction {
    fn from(args: FillExactInArgs) -> Self {
        Self {
            taker_side: args.taker_side,
            amount_in_atoms: args.amount_in_atoms,
            params: args.params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillAccounts {
    pub user: Pubkey,
    pub fill_authority: Pubkey,
    pub user_base_token_account: Pubkey,
    pub user_quote_token_account: Pubkey,
    pub maker_base_token_account: Pubkey,
    pub maker_quote_token_account: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub quote_token_program: Pubkey,
    pub instructions_sysvar: Pubkey,
}

#[derive(Debug, Clone)]
pub struct FillAnalysis {
    pub taker_side: Side,
    pub amount_in_atoms: u64,
    pub amount_out_atoms: u64,
    pub vwap_ticks: u64,
    pub levels_consumed: usize,
    pub total_lots_filled: u64,
    pub amount_spent_atoms: u64,
    pub lot_size_base: u64,
    pub tick_size_qpb: u64,
}

impl FillAnalysis {
    pub fn effective_price(&self) -> f64 {
        if self.amount_out_atoms == 0 {
            return 0.0;
        }
        if matches!(self.taker_side, Side::Bid) {
            self.amount_spent_atoms as f64 / self.amount_out_atoms as f64
        } else {
            self.amount_out_atoms as f64 / self.amount_spent_atoms as f64
        }
    }
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Bid => write!(f, "Bid"),
            Side::Ask => write!(f, "Ask"),
        }
    }
}

impl std::fmt::Display for FillAnalysis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FillAnalysis {{ side: {}, in: {}, spent: {}, out: {}, vwap_ticks: {}, \
             levels: {}, lots: {}, eff_price: {:.6} }}",
            self.taker_side,
            self.amount_in_atoms,
            self.amount_spent_atoms,
            self.amount_out_atoms,
            self.vwap_ticks,
            self.levels_consumed,
            self.total_lots_filled,
            self.effective_price(),
        )
    }
}
