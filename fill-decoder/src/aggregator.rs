//! Jupiter aggregator instruction decoding via Borsh.
//!
//! Supports all Jupiter route instruction variants and extracts embedded
//! RFQ v2 `fill_exact_in` data from `JupiterRfqV2` swap steps.

use borsh::BorshDeserialize;

use crate::analysis::analyze_fill;
use crate::types::{FillAnalysis, FillExactInInstruction, FillExactInParams, Side};

/// Base-58 encoded Jupiter aggregator program ID.
pub const JUPITER_PROGRAM_ID: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

/// The Jupiter aggregator IDL, embedded at compile time.
pub const AGGREGATOR_IDL_JSON: &str = include_str!("../idls/aggregator.json");

const ROUTE: [u8; 8] = [229, 23, 203, 151, 122, 227, 173, 42];
const ROUTE_WITH_TOKEN_LEDGER: [u8; 8] = [150, 86, 71, 116, 167, 93, 14, 104];
const EXACT_OUT_ROUTE: [u8; 8] = [208, 51, 239, 151, 123, 43, 237, 92];
const SHARED_ACCOUNTS_ROUTE: [u8; 8] = [193, 32, 155, 51, 65, 214, 156, 129];
const SHARED_ACCOUNTS_EXACT_OUT_ROUTE: [u8; 8] = [176, 209, 105, 168, 154, 125, 69, 62];
const SHARED_ACCOUNTS_ROUTE_WITH_TOKEN_LEDGER: [u8; 8] = [230, 121, 143, 80, 119, 159, 106, 170];
const ROUTE_V2: [u8; 8] = [187, 100, 250, 204, 49, 196, 175, 20];
const EXACT_OUT_ROUTE_V2: [u8; 8] = [157, 138, 184, 82, 21, 244, 243, 36];
const SHARED_ACCOUNTS_ROUTE_V2: [u8; 8] = [209, 152, 83, 147, 124, 254, 216, 233];
const SHARED_ACCOUNTS_EXACT_OUT_ROUTE_V2: [u8; 8] = [53, 96, 229, 202, 216, 187, 250, 24];

#[derive(Debug, Clone, BorshDeserialize)]
#[allow(dead_code)]
struct RemainingAccountsSlice {
    accounts_type: u8,
    length: u8,
}

#[derive(Debug, Clone, BorshDeserialize)]
#[allow(dead_code)]
struct RemainingAccountsInfo {
    slices: Vec<RemainingAccountsSlice>,
}

/// Jupiter `Side` enum (separate from the RFQ v2 `Side` — same layout).
#[derive(Debug, Clone, BorshDeserialize)]
enum JupSide {
    Bid,
    Ask,
}

#[derive(Debug, Clone, BorshDeserialize)]
#[allow(dead_code)]
enum CandidateSwapResult {
    OutAmount(u64),
    ProgramError(u64),
}

#[derive(Debug, Clone, BorshDeserialize)]
#[allow(dead_code)]
enum CandidateSwap {
    HumidiFi { swap_id: u64, is_base_to_quote: bool },
    TesseraV { side: JupSide },
    HumidiFiV2 { swap_id: u64, is_base_to_quote: bool },
}

/// All 126 Swap enum variants from the Jupiter aggregator IDL.
#[derive(Debug, Clone, BorshDeserialize)]
#[allow(dead_code)]
enum Swap {
    Saber,                                              // 0
    SaberAddDecimalsDeposit,                            // 1
    SaberAddDecimalsWithdraw,                           // 2
    TokenSwap,                                          // 3
    Sencha,                                             // 4
    Step,                                               // 5
    Cropper,                                            // 6
    Raydium,                                            // 7
    Crema { a_to_b: bool },                             // 8
    Lifinity,                                           // 9
    Mercurial,                                          // 10
    Cykura,                                             // 11
    Serum { side: JupSide },                            // 12
    MarinadeDeposit,                                    // 13
    MarinadeUnstake,                                    // 14
    Aldrin { side: JupSide },                           // 15
    AldrinV2 { side: JupSide },                         // 16
    Whirlpool { a_to_b: bool },                         // 17
    Invariant { x_to_y: bool },                         // 18
    Meteora,                                            // 19
    GooseFX,                                            // 20
    DeltaFi { stable: bool },                           // 21
    Balansol,                                           // 22
    MarcoPolo { x_to_y: bool },                         // 23
    Dradex { side: JupSide },                           // 24
    LifinityV2,                                         // 25
    RaydiumClmm,                                        // 26
    Openbook { side: JupSide },                         // 27
    Phoenix { side: JupSide },                          // 28
    Symmetry { from_token_id: u64, to_token_id: u64 },  // 29
    TokenSwapV2,                                        // 30
    HeliumTreasuryManagementRedeemV0,                   // 31
    StakeDexStakeWrappedSol,                            // 32
    StakeDexSwapViaStake { bridge_stake_seed: u32 },    // 33
    GooseFXV2,                                          // 34
    Perps,                                              // 35
    PerpsAddLiquidity,                                  // 36
    PerpsRemoveLiquidity,                               // 37
    MeteoraDlmm,                                       // 38
    OpenBookV2 { side: JupSide },                       // 39
    RaydiumClmmV2,                                     // 40
    StakeDexPrefundWithdrawStakeAndDepositStake { bridge_stake_seed: u32 }, // 41
    Clone { pool_index: u8, quantity_is_input: bool, quantity_is_collateral: bool }, // 42
    SanctumS { src_lst_value_calc_accs: u8, dst_lst_value_calc_accs: u8, src_lst_index: u32, dst_lst_index: u32 }, // 43
    SanctumSAddLiquidity { lst_value_calc_accs: u8, lst_index: u32 }, // 44
    SanctumSRemoveLiquidity { lst_value_calc_accs: u8, lst_index: u32 }, // 45
    RaydiumCP,                                          // 46
    WhirlpoolSwapV2 { a_to_b: bool, remaining_accounts_info: Option<RemainingAccountsInfo> }, // 47
    OneIntro,                                           // 48
    PumpWrappedBuy,                                     // 49
    PumpWrappedSell,                                    // 50
    PerpsV2,                                            // 51
    PerpsV2AddLiquidity,                                // 52
    PerpsV2RemoveLiquidity,                             // 53
    MoonshotWrappedBuy,                                 // 54
    MoonshotWrappedSell,                                // 55
    StabbleStableSwap,                                  // 56
    StabbleWeightedSwap,                                // 57
    Obric { x_to_y: bool },                             // 58
    FoxBuyFromEstimatedCost,                            // 59
    FoxClaimPartial { is_y: bool },                      // 60
    SolFi { is_quote_to_base: bool },                    // 61
    SolayerDelegateNoInit,                              // 62
    SolayerUndelegateNoInit,                            // 63
    TokenMill { side: JupSide },                         // 64
    DaosFunBuy,                                         // 65
    DaosFunSell,                                        // 66
    ZeroFi,                                             // 67
    StakeDexWithdrawWrappedSol,                         // 68
    VirtualsBuy,                                        // 69
    VirtualsSell,                                       // 70
    Perena { in_index: u8, out_index: u8 },              // 71
    PumpSwapBuy,                                        // 72
    PumpSwapSell,                                       // 73
    Gamma,                                              // 74
    MeteoraDlmmSwapV2 { remaining_accounts_info: RemainingAccountsInfo }, // 75
    Woofi,                                              // 76
    MeteoraDammV2,                                      // 77
    MeteoraDynamicBondingCurveSwap,                     // 78
    StabbleStableSwapV2,                                // 79
    StabbleWeightedSwapV2,                              // 80
    RaydiumLaunchlabBuy { share_fee_rate: u64 },         // 81
    RaydiumLaunchlabSell { share_fee_rate: u64 },        // 82
    BoopdotfunWrappedBuy,                               // 83
    BoopdotfunWrappedSell,                              // 84
    Plasma { side: JupSide },                            // 85
    GoonFi { is_bid: bool, blacklist_bump: u8 },         // 86
    HumidiFi { swap_id: u64, is_base_to_quote: bool },   // 87
    MeteoraDynamicBondingCurveSwapWithRemainingAccounts, // 88
    TesseraV { side: JupSide },                          // 89
    PumpWrappedBuyV2,                                   // 90
    PumpWrappedSellV2,                                  // 91
    PumpSwapBuyV2,                                      // 92
    PumpSwapSellV2,                                     // 93
    Heaven { a_to_b: bool },                             // 94
    SolFiV2 { is_quote_to_base: bool },                  // 95
    Aquifer,                                            // 96
    PumpWrappedBuyV3,                                   // 97
    PumpWrappedSellV3,                                  // 98
    PumpSwapBuyV3,                                      // 99
    PumpSwapSellV3,                                     // 100
    JupiterLendDeposit,                                 // 101
    JupiterLendRedeem,                                  // 102
    DefiTuna { a_to_b: bool, remaining_accounts_info: Option<RemainingAccountsInfo> }, // 103
    AlphaQ { a_to_b: bool },                             // 104
    RaydiumV2,                                          // 105
    SarosDlmm { swap_for_y: bool },                      // 106
    Futarchy { side: JupSide },                          // 107
    MeteoraDammV2WithRemainingAccounts,                  // 108
    Obsidian,                                           // 109
    WhaleStreet { side: JupSide },                       // 110
    DynamicV1 { candidate_swaps: Vec<CandidateSwap>, best_position: Option<u8> }, // 111
    PumpWrappedBuyV4,                                   // 112
    PumpWrappedSellV4,                                  // 113
    CarrotIssue,                                        // 114
    CarrotRedeem,                                       // 115
    Manifest { side: JupSide },                          // 116
    BisonFi { a_to_b: bool },                            // 117
    HumidiFiV2 { swap_id: u64, is_base_to_quote: bool }, // 118
    PerenaStar { is_mint: bool },                        // 119
    JupiterRfqV2 { side: JupSide, fill_data: Vec<u8> },  // 120
    GoonFiV2 { is_bid: bool },                           // 121
    Scorch { swap_id: u128 },                            // 122
    VaultLiquidUnstake { lst_amounts: [u64; 5], seed: u64 }, // 123
    XOrca,                                              // 124
    Quantum { side: JupSide },                           // 125
}

#[derive(Debug, Clone, BorshDeserialize)]
#[allow(dead_code)]
struct RoutePlanStep {
    swap: Swap,
    percent: u8,
    input_index: u8,
    output_index: u8,
}

#[derive(Debug, Clone, BorshDeserialize)]
#[allow(dead_code)]
struct RoutePlanStepV2 {
    swap: Swap,
    bps: u16,
    input_index: u8,
    output_index: u8,
}

#[derive(BorshDeserialize)]
struct RouteArgs {
    route_plan: Vec<RoutePlanStep>,
    in_amount: u64,
    #[allow(dead_code)] quoted_out_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u8,
}

#[derive(BorshDeserialize)]
struct RouteWithTokenLedgerArgs {
    route_plan: Vec<RoutePlanStep>,
    #[allow(dead_code)] quoted_out_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u8,
}

#[derive(BorshDeserialize)]
struct ExactOutRouteArgs {
    route_plan: Vec<RoutePlanStep>,
    #[allow(dead_code)] out_amount: u64,
    #[allow(dead_code)] quoted_in_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u8,
}

#[derive(BorshDeserialize)]
struct SharedAccountsRouteArgs {
    #[allow(dead_code)] id: u8,
    route_plan: Vec<RoutePlanStep>,
    in_amount: u64,
    #[allow(dead_code)] quoted_out_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u8,
}

#[derive(BorshDeserialize)]
struct SharedAccountsExactOutRouteArgs {
    #[allow(dead_code)] id: u8,
    route_plan: Vec<RoutePlanStep>,
    #[allow(dead_code)] out_amount: u64,
    #[allow(dead_code)] quoted_in_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u8,
}

#[derive(BorshDeserialize)]
struct SharedAccountsRouteWithTokenLedgerArgs {
    #[allow(dead_code)] id: u8,
    route_plan: Vec<RoutePlanStep>,
    #[allow(dead_code)] quoted_out_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u8,
}

#[derive(BorshDeserialize)]
struct RouteV2Args {
    in_amount: u64,
    #[allow(dead_code)] quoted_out_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u16,
    #[allow(dead_code)] positive_slippage_bps: u16,
    route_plan: Vec<RoutePlanStepV2>,
}

#[derive(BorshDeserialize)]
struct ExactOutRouteV2Args {
    #[allow(dead_code)] out_amount: u64,
    #[allow(dead_code)] quoted_in_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u16,
    #[allow(dead_code)] positive_slippage_bps: u16,
    route_plan: Vec<RoutePlanStepV2>,
}

#[derive(BorshDeserialize)]
struct SharedAccountsRouteV2Args {
    #[allow(dead_code)] id: u8,
    in_amount: u64,
    #[allow(dead_code)] quoted_out_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u16,
    #[allow(dead_code)] positive_slippage_bps: u16,
    route_plan: Vec<RoutePlanStepV2>,
}

#[derive(BorshDeserialize)]
struct SharedAccountsExactOutRouteV2Args {
    #[allow(dead_code)] id: u8,
    #[allow(dead_code)] out_amount: u64,
    #[allow(dead_code)] quoted_in_amount: u64,
    #[allow(dead_code)] slippage_bps: u16,
    #[allow(dead_code)] platform_fee_bps: u16,
    #[allow(dead_code)] positive_slippage_bps: u16,
    route_plan: Vec<RoutePlanStepV2>,
}

/// Returns `true` if `instruction_data` begins with a known Jupiter route discriminator.
pub fn is_jupiter_route(instruction_data: &[u8]) -> bool {
    if instruction_data.len() < 8 {
        return false;
    }
    let disc: [u8; 8] = instruction_data[..8].try_into().unwrap();
    matches!(
        disc,
        ROUTE
            | ROUTE_WITH_TOKEN_LEDGER
            | EXACT_OUT_ROUTE
            | SHARED_ACCOUNTS_ROUTE
            | SHARED_ACCOUNTS_EXACT_OUT_ROUTE
            | SHARED_ACCOUNTS_ROUTE_WITH_TOKEN_LEDGER
            | ROUTE_V2
            | EXACT_OUT_ROUTE_V2
            | SHARED_ACCOUNTS_ROUTE_V2
            | SHARED_ACCOUNTS_EXACT_OUT_ROUTE_V2
    )
}

/// Try to decode an RFQ v2 fill embedded inside a Jupiter route instruction.
///
/// Returns `None` if the data is not a Jupiter route or does not contain a
/// `JupiterRfqV2` swap step.
///
/// It properly deserializes the Jupiter route instruction using Borsh,
/// then extracts the `fill_data` from any `JupiterRfqV2` step.
///
/// Three strategies for decoding `fill_data`:
///
/// 1. `fill_data` is a Borsh-serialized `FillExactInInstruction` (side + amount + params)
/// 2. `fill_data` starts with discriminator + `FillExactInInstruction`
/// 3. `fill_data` is a Borsh-serialized `FillExactInParams` → combine with side/amount
pub fn decode_jupiter_rfq_fill(data: &[u8]) -> Option<(FillExactInInstruction, FillAnalysis)> {
    if data.len() < 8 {
        return None;
    }
    let disc: [u8; 8] = data[..8].try_into().unwrap();
    let args = &data[8..];

    // Extract route plan steps and in_amount from each instruction variant.
    let (steps, in_amount) = match disc {
        ROUTE => {
            let a = borsh::from_slice::<RouteArgs>(args).ok()?;
            (extract_rfq_steps_v1(&a.route_plan), Some(a.in_amount))
        }
        ROUTE_WITH_TOKEN_LEDGER => {
            let a = borsh::from_slice::<RouteWithTokenLedgerArgs>(args).ok()?;
            (extract_rfq_steps_v1(&a.route_plan), None)
        }
        EXACT_OUT_ROUTE => {
            let a = borsh::from_slice::<ExactOutRouteArgs>(args).ok()?;
            (extract_rfq_steps_v1(&a.route_plan), None)
        }
        SHARED_ACCOUNTS_ROUTE => {
            let a = borsh::from_slice::<SharedAccountsRouteArgs>(args).ok()?;
            (extract_rfq_steps_v1(&a.route_plan), Some(a.in_amount))
        }
        SHARED_ACCOUNTS_EXACT_OUT_ROUTE => {
            let a = borsh::from_slice::<SharedAccountsExactOutRouteArgs>(args).ok()?;
            (extract_rfq_steps_v1(&a.route_plan), None)
        }
        SHARED_ACCOUNTS_ROUTE_WITH_TOKEN_LEDGER => {
            let a = borsh::from_slice::<SharedAccountsRouteWithTokenLedgerArgs>(args).ok()?;
            (extract_rfq_steps_v1(&a.route_plan), None)
        }
        ROUTE_V2 => {
            let a = borsh::from_slice::<RouteV2Args>(args).ok()?;
            (extract_rfq_steps_v2(&a.route_plan), Some(a.in_amount))
        }
        EXACT_OUT_ROUTE_V2 => {
            let a = borsh::from_slice::<ExactOutRouteV2Args>(args).ok()?;
            (extract_rfq_steps_v2(&a.route_plan), None)
        }
        SHARED_ACCOUNTS_ROUTE_V2 => {
            let a = borsh::from_slice::<SharedAccountsRouteV2Args>(args).ok()?;
            (extract_rfq_steps_v2(&a.route_plan), Some(a.in_amount))
        }
        SHARED_ACCOUNTS_EXACT_OUT_ROUTE_V2 => {
            let a = borsh::from_slice::<SharedAccountsExactOutRouteV2Args>(args).ok()?;
            (extract_rfq_steps_v2(&a.route_plan), None)
        }
        _ => return None,
    };

    if steps.is_empty() {
        return None;
    }

    // Try to decode the fill from the first JupiterRfqV2 step found.
    for (side, fill_data) in &steps {
        if let Some(result) = try_decode_rfq_fill(side, fill_data, in_amount) {
            return Some(result);
        }
    }

    None
}

/// Extract `(side, fill_data)` from v1 route plan steps.
fn extract_rfq_steps_v1(plan: &[RoutePlanStep]) -> Vec<(JupSide, Vec<u8>)> {
    plan.iter()
        .filter_map(|step| match &step.swap {
            Swap::JupiterRfqV2 { side, fill_data } => Some((side.clone(), fill_data.clone())),
            _ => None,
        })
        .collect()
}

/// Extract `(side, fill_data)` from v2 route plan steps.
fn extract_rfq_steps_v2(plan: &[RoutePlanStepV2]) -> Vec<(JupSide, Vec<u8>)> {
    plan.iter()
        .filter_map(|step| match &step.swap {
            Swap::JupiterRfqV2 { side, fill_data } => Some((side.clone(), fill_data.clone())),
            _ => None,
        })
        .collect()
}

/// Convert Jupiter `Side` to RFQ v2 `Side`.
fn to_rfq_side(side: &JupSide) -> Side {
    match side {
        JupSide::Bid => Side::Bid,
        JupSide::Ask => Side::Ask,
    }
}

/// Try three strategies to decode the `fill_data` bytes.
fn try_decode_rfq_fill(
    jup_side: &JupSide,
    fill_data: &[u8],
    in_amount: Option<u64>,
) -> Option<(FillExactInInstruction, FillAnalysis)> {
    let side = to_rfq_side(jup_side);

    // Strategy 1: fill_data is a full FillExactInInstruction (side + amount + params).
    if let Ok(ix) = borsh::from_slice::<FillExactInInstruction>(fill_data) {
        if let Ok(analysis) = analyze_fill(&ix) {
            if analysis.levels_consumed > 0 {
                return Some((ix, analysis));
            }
        }
    }

    // Strategy 2: fill_data starts with the 8-byte discriminator + FillExactInInstruction.
    if fill_data.len() > 8 {
        if let Ok(ix) = borsh::from_slice::<FillExactInInstruction>(&fill_data[8..]) {
            if let Ok(analysis) = analyze_fill(&ix) {
                if analysis.levels_consumed > 0 {
                    return Some((ix, analysis));
                }
            }
        }
    }

    // Strategy 3: fill_data is just FillExactInParams → combine with side/amount.
    if let Ok(params) = borsh::from_slice::<FillExactInParams>(fill_data) {
        let amount = in_amount.unwrap_or(0);
        let ix = FillExactInInstruction {
            taker_side: side,
            amount_in_atoms: amount,
            params,
        };
        if let Ok(analysis) = analyze_fill(&ix) {
            if analysis.levels_consumed > 0 {
                return Some((ix, analysis));
            }
        }
    }

    None
}
