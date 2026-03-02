//! Off-chain sweep simulation for `fill_exact_in` instructions.

use crate::error::FillDecoderError;
use crate::types::{FillAnalysis, FillExactInInstruction, Side};

/// Run the off-chain sweep simulation on a decoded `fill_exact_in` instruction.
///
/// Mirrors the on-chain logic: iterate through levels best-to-worst and fill
/// as many lots as the input amount allows.
///
/// - **Bid** (taker buys base): input = quote-atoms, output = base-atoms.
/// - **Ask** (taker sells base): input = base-atoms, output = quote-atoms.
pub fn analyze_fill(ix: &FillExactInInstruction) -> crate::Result<FillAnalysis> {
    let tick = ix.params.tick_size_qpb;
    let lot = ix.params.lot_size_base;

    if tick == 0 || lot == 0 {
        return Err(FillDecoderError::validation(
            "tick_size_qpb and lot_size_base must be > 0",
        ));
    }

    let mut remaining = ix.amount_in_atoms;
    let mut total_out: u64 = 0;
    let mut total_lots: u64 = 0;
    let mut weighted_px_sum: u128 = 0;
    let mut levels_consumed: usize = 0;

    for level in &ix.params.levels {
        if remaining == 0 {
            break;
        }
        levels_consumed += 1;

        let price_per_lot = level
            .px_ticks
            .checked_mul(tick)
            .ok_or_else(|| FillDecoderError::other("price_per_lot overflow"))?;

        match ix.taker_side {
            Side::Bid => {
                // Taker pays quote, receives base.
                if price_per_lot == 0 {
                    continue;
                }
                let affordable = remaining / price_per_lot;
                let lots = affordable.min(level.qty_lots);
                let cost = lots
                    .checked_mul(price_per_lot)
                    .ok_or_else(|| FillDecoderError::other("cost overflow"))?;
                let base_out = lots
                    .checked_mul(lot)
                    .ok_or_else(|| FillDecoderError::other("base_out overflow"))?;
                remaining -= cost;
                total_out += base_out;
                total_lots += lots;
                weighted_px_sum += level.px_ticks as u128 * lots as u128;
            }
            Side::Ask => {
                // Taker pays base, receives quote.
                let affordable = remaining / lot;
                let lots = affordable.min(level.qty_lots);
                let base_cost = lots
                    .checked_mul(lot)
                    .ok_or_else(|| FillDecoderError::other("base_cost overflow"))?;
                let quote_out = lots
                    .checked_mul(price_per_lot)
                    .ok_or_else(|| FillDecoderError::other("quote_out overflow"))?;
                remaining -= base_cost;
                total_out += quote_out;
                total_lots += lots;
                weighted_px_sum += level.px_ticks as u128 * lots as u128;
            }
        }
    }

    let vwap_ticks = if total_lots > 0 {
        (weighted_px_sum / total_lots as u128) as u64
    } else {
        0
    };

    let amount_spent = ix.amount_in_atoms - remaining;

    Ok(FillAnalysis {
        taker_side: ix.taker_side,
        amount_in_atoms: ix.amount_in_atoms,
        amount_out_atoms: total_out,
        vwap_ticks,
        levels_consumed,
        total_lots_filled: total_lots,
        amount_spent_atoms: amount_spent,
        lot_size_base: lot,
        tick_size_qpb: tick,
    })
}
