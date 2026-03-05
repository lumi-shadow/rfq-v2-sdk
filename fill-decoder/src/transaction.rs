
use solana_sdk::message::VersionedMessage;
use solana_sdk::transaction::VersionedTransaction;
use std::collections::HashMap;
use serde::Serialize;

use crate::analysis::analyze_fill;
use crate::decode::{
    decode_fill_instruction, decode_fill_params,
    is_fill_exact_in, FILL_ACCOUNT_LABELS, RFQ_V2_PROGRAM_ID,
};
use crate::error::FillDecoderError;
use crate::jupiter::{is_jupiter_route, swap_kind_name, DecodedJupiterRoute, JUPITER_PROGRAM_ID};
use crate::types::{FillAnalysis, FillExactInInstruction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageVersion {
    Legacy,
    V0,
}

impl std::fmt::Display for MessageVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legacy => write!(f, "Legacy"),
            Self::V0 => write!(f, "V0"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    pub num_required_signatures: u8,
    pub num_readonly_signed_accounts: u8,
    pub num_readonly_unsigned_accounts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAccount {
    pub index: u8,
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DecodedInstruction {
    pub instruction_index: usize,
    pub program_id_index: u8,
    pub program_id: String,
    pub account_indices: Vec<u8>,
    pub accounts: Vec<ResolvedAccount>,
    pub data: Vec<u8>,
    pub fill: Option<(FillExactInInstruction, FillAnalysis)>,
    pub jupiter: Option<DecodedJupiterInstruction>,
}

#[derive(Debug, Clone)]
pub struct DecodedJupiterInstruction {
    pub kind: String,
    pub in_amount: u64,
    pub quoted_out_amount: u64,
    pub slippage_bps: u16,
    pub platform_fee_bps: u16,
    pub positive_slippage_bps: Option<u16>,
    pub hops: Vec<DecodedJupiterHop>,
}

#[derive(Debug, Clone)]
pub struct DecodedJupiterHop {
    pub hop_index: usize,
    pub swap: String,
    pub input_index: u8,
    pub output_index: u8,
    pub share_bps: u16,
    pub estimated_in_amount: Option<u64>,
    pub estimated_out_amount: Option<u64>,
    pub rfq_fill: Option<(FillExactInInstruction, FillAnalysis)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressTableLookup {
    pub account_key: String,
    pub writable_indexes: Vec<u8>,
    pub readonly_indexes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DecodedMessage {
    pub version: MessageVersion,
    pub header: MessageHeader,
    pub account_keys: Vec<String>,
    pub recent_blockhash: String,
    pub instructions: Vec<DecodedInstruction>,
    pub address_table_lookups: Vec<AddressTableLookup>,
}

#[derive(Debug, Clone)]
pub struct DecodedTransaction {
    pub signatures: Vec<String>,
    pub message: DecodedMessage,
}

#[derive(Debug, Clone, Serialize)]
pub struct MmTxSummary {
    pub signatures: Vec<String>,
    pub message_version: String,
    pub instructions: Vec<MmInstructionSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MmInstructionSummary {
    pub instruction_index: usize,
    pub program_id: String,
    pub data: Vec<u8>,
    pub fill: Option<MmFillSummary>,
    pub jupiter: Option<MmJupiterSummary>,
    pub transfer: Option<MmTransferSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MmFillSummary {
    pub taker_side: String,
    pub amount_in_atoms: u64,
    pub amount_spent_atoms: u64,
    pub amount_out_atoms: u64,
    pub vwap_ticks: u64,
    pub levels_consumed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MmJupiterSummary {
    pub kind: String,
    pub in_amount: u64,
    pub quoted_out_amount: u64,
    pub slippage_bps: u16,
    pub platform_fee_bps: u16,
    pub positive_slippage_bps: Option<u16>,
    pub hops: Vec<MmJupiterHopSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MmJupiterHopSummary {
    pub hop_index: usize,
    pub swap: String,
    pub input_index: u8,
    pub output_index: u8,
    pub share_bps: u16,
    pub estimated_in_amount: Option<u64>,
    pub estimated_out_amount: Option<u64>,
    pub rfq_fill: Option<MmFillSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MmTransferSummary {
    pub kind: String,
    pub source: String,
    pub destination: String,
    pub authority: Option<String>,
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn decode_transaction_base64(input: &str) -> crate::Result<DecodedTransaction> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = STANDARD
        .decode(input)
        .map_err(|e| FillDecoderError::other(format!("invalid base64: {e}")))?;
    decode_transaction_bytes(&bytes)
}

pub fn decode_mm_tx_base64(input: &str) -> crate::Result<MmTxSummary> {
    let tx = decode_transaction_base64(input)?;
    Ok(filter_mm_summary(mm_summary_from_decoded_tx(&tx)))
}

pub fn decode_mm_tx_base64_json(input: &str) -> crate::Result<String> {
    let summary = decode_mm_tx_base64(input)?;
    serde_json::to_string_pretty(&summary)
        .map_err(|e| FillDecoderError::other(format!("failed to serialize MM summary JSON: {e}")))
}

pub fn mm_summary_from_decoded_tx(tx: &DecodedTransaction) -> MmTxSummary {
    let instructions = tx
        .message
        .instructions
        .iter()
        .map(|ix| MmInstructionSummary {
            instruction_index: ix.instruction_index,
            program_id: ix.program_id.clone(),
            data: ix.data.clone(),
            fill: ix.fill.as_ref().map(|(fill, analysis)| MmFillSummary {
                taker_side: fill.taker_side.to_string(),
                amount_in_atoms: fill.amount_in_atoms,
                amount_spent_atoms: analysis.amount_spent_atoms,
                amount_out_atoms: analysis.amount_out_atoms,
                vwap_ticks: analysis.vwap_ticks,
                levels_consumed: analysis.levels_consumed,
            }),
            jupiter: ix.jupiter.as_ref().map(|j| MmJupiterSummary {
                kind: j.kind.clone(),
                in_amount: j.in_amount,
                quoted_out_amount: j.quoted_out_amount,
                slippage_bps: j.slippage_bps,
                platform_fee_bps: j.platform_fee_bps,
                positive_slippage_bps: j.positive_slippage_bps,
                hops: j
                    .hops
                    .iter()
                    .map(|h| MmJupiterHopSummary {
                        hop_index: h.hop_index,
                        swap: h.swap.clone(),
                        input_index: h.input_index,
                        output_index: h.output_index,
                        share_bps: h.share_bps,
                        estimated_in_amount: h.estimated_in_amount,
                        estimated_out_amount: h.estimated_out_amount,
                        rfq_fill: h.rfq_fill.as_ref().map(|(fill, analysis)| MmFillSummary {
                            taker_side: fill.taker_side.to_string(),
                            amount_in_atoms: fill.amount_in_atoms,
                            amount_spent_atoms: analysis.amount_spent_atoms,
                            amount_out_atoms: analysis.amount_out_atoms,
                            vwap_ticks: analysis.vwap_ticks,
                            levels_consumed: analysis.levels_consumed,
                        }),
                    })
                    .collect(),
            }),
            transfer: extract_transfer_summary(ix),
        })
        .collect();

    MmTxSummary {
        signatures: tx.signatures.clone(),
        message_version: tx.message.version.to_string(),
        instructions,
    }
}

pub fn filter_mm_summary(mut summary: MmTxSummary) -> MmTxSummary {
    summary.instructions.retain(|ix| {
        ix.fill.is_some() || ix.jupiter.is_some() || ix.transfer.is_some()
    });
    summary
}

fn read_u32_le(data: &[u8], start: usize) -> Option<u32> {
    let end = start.checked_add(4)?;
    let bytes = data.get(start..end)?;
    let arr: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_le_bytes(arr))
}

fn extract_transfer_summary(ix: &DecodedInstruction) -> Option<MmTransferSummary> {
    const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    const SPL_TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
    const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

    if ix.program_id == SPL_TOKEN_PROGRAM_ID || ix.program_id == SPL_TOKEN_2022_PROGRAM_ID {
        let tag = *ix.data.first()?;
        match tag {
            3 => {
                let source = ix.accounts.first()?.pubkey.clone();
                let destination = ix.accounts.get(1)?.pubkey.clone();
                let authority = ix.accounts.get(2).map(|a| a.pubkey.clone());
                Some(MmTransferSummary {
                    kind: "spl_token_transfer".to_string(),
                    source,
                    destination,
                    authority,
                })
            }
            12 => {
                let source = ix.accounts.first()?.pubkey.clone();
                let destination = ix.accounts.get(2)?.pubkey.clone();
                let authority = ix.accounts.get(3).map(|a| a.pubkey.clone());
                Some(MmTransferSummary {
                    kind: "spl_token_transfer_checked".to_string(),
                    source,
                    destination,
                    authority,
                })
            }
            9 => {
                let source = ix.accounts.first()?.pubkey.clone();
                let destination = ix.accounts.get(1)?.pubkey.clone();
                let authority = ix.accounts.get(2).map(|a| a.pubkey.clone());
                Some(MmTransferSummary {
                    kind: "spl_token_close_account".to_string(),
                    source,
                    destination,
                    authority,
                })
            }
            _ => None,
        }
    } else if ix.program_id == SYSTEM_PROGRAM_ID {
        if read_u32_le(&ix.data, 0) == Some(2) {
            let source = ix.accounts.first()?.pubkey.clone();
            let destination = ix.accounts.get(1)?.pubkey.clone();
            Some(MmTransferSummary {
                kind: "system_transfer".to_string(),
                source,
                destination,
                authority: None,
            })
        } else {
            None
        }
    } else {
        None
    }
}

pub fn decode_transaction_bytes(data: &[u8]) -> crate::Result<DecodedTransaction> {
    let tx: VersionedTransaction = bincode::deserialize(data)
        .map_err(|e| FillDecoderError::other(format!("failed to deserialize transaction: {e}")))?;

    let signatures: Vec<String> = tx
        .signatures
        .iter()
        .map(|s| bs58::encode(s).into_string())
        .collect();

    let message = decode_versioned_message(&tx.message)?;

    Ok(DecodedTransaction { signatures, message })
}

pub fn decode_message_base64(input: &str) -> crate::Result<DecodedMessage> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = STANDARD
        .decode(input)
        .map_err(|e| FillDecoderError::other(format!("invalid base64: {e}")))?;

    let msg: VersionedMessage = bincode::deserialize(&bytes)
        .map_err(|e| FillDecoderError::other(format!("failed to deserialize message: {e}")))?;

    decode_versioned_message(&msg)
}

fn decode_versioned_message(msg: &VersionedMessage) -> crate::Result<DecodedMessage> {
    let (version, header, static_keys, recent_blockhash, instructions, address_table_lookups) =
        match msg {
            VersionedMessage::Legacy(legacy) => {
                let header = MessageHeader {
                    num_required_signatures: legacy.header.num_required_signatures,
                    num_readonly_signed_accounts: legacy.header.num_readonly_signed_accounts,
                    num_readonly_unsigned_accounts: legacy.header.num_readonly_unsigned_accounts,
                };
                (
                    MessageVersion::Legacy,
                    header,
                    &legacy.account_keys[..],
                    legacy.recent_blockhash,
                    &legacy.instructions[..],
                    Vec::new(),
                )
            }
            VersionedMessage::V0(v0) => {
                let header = MessageHeader {
                    num_required_signatures: v0.header.num_required_signatures,
                    num_readonly_signed_accounts: v0.header.num_readonly_signed_accounts,
                    num_readonly_unsigned_accounts: v0.header.num_readonly_unsigned_accounts,
                };
                let lookups: Vec<AddressTableLookup> = v0
                    .address_table_lookups
                    .iter()
                    .map(|l| AddressTableLookup {
                        account_key: bs58::encode(&l.account_key).into_string(),
                        writable_indexes: l.writable_indexes.clone(),
                        readonly_indexes: l.readonly_indexes.clone(),
                    })
                    .collect();
                (
                    MessageVersion::V0,
                    header,
                    &v0.account_keys[..],
                    v0.recent_blockhash,
                    &v0.instructions[..],
                    lookups,
                )
            }
        };

    let num_static = static_keys.len();
    let num_required_signatures = header.num_required_signatures as usize;
    let num_readonly_signed = header.num_readonly_signed_accounts as usize;
    let num_readonly_unsigned = header.num_readonly_unsigned_accounts as usize;

    let total_writable_lookups: usize = address_table_lookups
        .iter()
        .map(|l| l.writable_indexes.len())
        .sum();
    let writable_lookup_end = num_static + total_writable_lookups;

    let account_keys: Vec<String> = static_keys
        .iter()
        .map(|k| bs58::encode(k).into_string())
        .collect();

    let recent_blockhash_str = bs58::encode(&recent_blockhash).into_string();

    let resolve = |idx: u8| -> ResolvedAccount {
        let i = idx as usize;
        if i < num_static {
            let is_signer = i < num_required_signatures;
            let is_writable = if is_signer {
                i < num_required_signatures.saturating_sub(num_readonly_signed)
            } else {
                i < num_static.saturating_sub(num_readonly_unsigned)
            };
            ResolvedAccount {
                index: idx,
                pubkey: account_keys[i].clone(),
                is_signer,
                is_writable,
                label: None,
            }
        } else if i < writable_lookup_end {
            ResolvedAccount {
                index: idx,
                pubkey: format!("LookupWritable[{}]", i - num_static),
                is_signer: false,
                is_writable: true,
                label: None,
            }
        } else {
            ResolvedAccount {
                index: idx,
                pubkey: format!("LookupReadonly[{}]", i - writable_lookup_end),
                is_signer: false,
                is_writable: false,
                label: None,
            }
        }
    };

    let decoded_instructions: Vec<DecodedInstruction> = instructions
        .iter()
        .enumerate()
        .map(|(ix_idx, ix)| {
            let program_id = if (ix.program_id_index as usize) < account_keys.len() {
                account_keys[ix.program_id_index as usize].clone()
            } else {
                format!("Unknown(index={})", ix.program_id_index)
            };

            let is_rfq_program = program_id == RFQ_V2_PROGRAM_ID;
            let is_jupiter_program = program_id == JUPITER_PROGRAM_ID.to_string();
            let is_direct_fill = is_fill_exact_in(&ix.data);
            let jupiter = if is_jupiter_program && is_jupiter_route(&ix.data) {
                decode_jupiter_instruction(&ix.data)
            } else {
                None
            };

            let fill = if is_direct_fill {
                decode_fill_instruction(&ix.data)
                    .ok()
                    .and_then(|fill_ix| analyze_fill(&fill_ix).ok().map(|a| (fill_ix, a)))
            } else {
                jupiter
                    .as_ref()
                    .and_then(|j| j.hops.iter().find_map(|h| h.rfq_fill.clone()))
            };

            let accounts: Vec<ResolvedAccount> = ix
                .accounts
                .iter()
                .enumerate()
                .map(|(pos, &idx)| {
                    let mut acct = resolve(idx);
                    if is_direct_fill && is_rfq_program && pos < FILL_ACCOUNT_LABELS.len() {
                        acct.label = Some(FILL_ACCOUNT_LABELS[pos].to_string());
                    }
                    acct
                })
                .collect();

            DecodedInstruction {
                instruction_index: ix_idx,
                program_id_index: ix.program_id_index,
                program_id,
                account_indices: ix.accounts.clone(),
                accounts,
                data: ix.data.clone(),
                fill,
                jupiter,
            }
        })
        .collect();

    Ok(DecodedMessage {
        version,
        header,
        account_keys,
        recent_blockhash: recent_blockhash_str,
        instructions: decoded_instructions,
        address_table_lookups,
    })
}

fn mul_div_u64(amount: u64, numerator: u64, denominator: u64) -> u64 {
    ((amount as u128) * (numerator as u128) / (denominator as u128)) as u64
}

fn decode_jupiter_instruction(data: &[u8]) -> Option<DecodedJupiterInstruction> {
    let route = DecodedJupiterRoute::decode(data)?;

    let mut known_amounts: HashMap<u8, u64> = HashMap::new();
    known_amounts.insert(0, route.in_amount());

    let mut hops = Vec::new();

    let mut push_hop = |hop_index: usize,
                        swap: &crate::jupiter::Swap,
                        input_index: u8,
                        output_index: u8,
                        share_bps: u16| {
        let estimated_in_amount = known_amounts
            .get(&input_index)
            .copied()
            .map(|amount| mul_div_u64(amount, share_bps as u64, 10_000));

        let mut estimated_out_amount = None;
        let mut rfq_fill = None;

        if let crate::jupiter::Swap::JupiterRfqV2 { side, fill_data } = swap {
            let params = decode_fill_params(fill_data).ok();
            if let Some(params) = params {
                let taker_side = match side {
                    crate::jupiter::Side::Bid => crate::types::Side::Bid,
                    crate::jupiter::Side::Ask => crate::types::Side::Ask,
                };
                let amount_in_atoms = estimated_in_amount.unwrap_or(route.in_amount());
                let fill_ix = FillExactInInstruction {
                    taker_side,
                    amount_in_atoms,
                    params,
                };
                if let Ok(analysis) = analyze_fill(&fill_ix) {
                    estimated_out_amount = Some(analysis.amount_out_atoms);
                    rfq_fill = Some((fill_ix, analysis));
                }
            }
        }

        if let Some(out) = estimated_out_amount {
            known_amounts
                .entry(output_index)
                .and_modify(|x| *x = x.saturating_add(out))
                .or_insert(out);
        }

        hops.push(DecodedJupiterHop {
            hop_index,
            swap: swap_kind_name(swap),
            input_index,
            output_index,
            share_bps,
            estimated_in_amount,
            estimated_out_amount,
            rfq_fill,
        });
    };

    match &route {
        DecodedJupiterRoute::Route { route_plan, .. } => {
            for (hop_index, step) in route_plan.iter().enumerate() {
                push_hop(
                    hop_index,
                    &step.swap,
                    step.input_index,
                    step.output_index,
                    (step.percent as u16) * 100,
                );
            }
        }
        DecodedJupiterRoute::SharedAccountsRoute { route_plan, .. } => {
            for (hop_index, step) in route_plan.iter().enumerate() {
                push_hop(
                    hop_index,
                    &step.swap,
                    step.input_index,
                    step.output_index,
                    (step.percent as u16) * 100,
                );
            }
        }
        DecodedJupiterRoute::RouteV2 { route_plan, .. } => {
            for (hop_index, step) in route_plan.iter().enumerate() {
                push_hop(
                    hop_index,
                    &step.swap,
                    step.input_index,
                    step.output_index,
                    step.bps,
                );
            }
        }
        DecodedJupiterRoute::SharedAccountsRouteV2 { route_plan, .. } => {
            for (hop_index, step) in route_plan.iter().enumerate() {
                push_hop(
                    hop_index,
                    &step.swap,
                    step.input_index,
                    step.output_index,
                    step.bps,
                );
            }
        }
    }

    Some(DecodedJupiterInstruction {
        kind: route.kind_name().to_string(),
        in_amount: route.in_amount(),
        quoted_out_amount: route.quoted_out_amount(),
        slippage_bps: route.slippage_bps(),
        platform_fee_bps: route.platform_fee_bps(),
        positive_slippage_bps: route.positive_slippage_bps(),
        hops,
    })
}

impl std::fmt::Display for DecodedTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Decoded Transaction ===")?;
        writeln!(f, "Signatures ({}):", self.signatures.len())?;
        for (i, sig) in self.signatures.iter().enumerate() {
            writeln!(f, "  [{}] {}", i, sig)?;
        }
        writeln!(f)?;
        write!(f, "{}", self.message)
    }
}

impl std::fmt::Display for DecodedMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Message Version: {}", self.version)?;
        writeln!(
            f,
            "Header: {} required sigs, {} readonly signed, {} readonly unsigned",
            self.header.num_required_signatures,
            self.header.num_readonly_signed_accounts,
            self.header.num_readonly_unsigned_accounts,
        )?;
        writeln!(f)?;

        writeln!(f, "Account Keys ({}):", self.account_keys.len())?;
        for (i, key) in self.account_keys.iter().enumerate() {
            let signer = i < self.header.num_required_signatures as usize;
            let writable = if signer {
                i < (self
                    .header
                    .num_required_signatures
                    .saturating_sub(self.header.num_readonly_signed_accounts))
                    as usize
            } else {
                i < self
                    .account_keys
                    .len()
                    .saturating_sub(self.header.num_readonly_unsigned_accounts as usize)
            };
            let flags = match (signer, writable) {
                (true, true) => "SIGNER WRITABLE",
                (true, false) => "SIGNER READONLY",
                (false, true) => "WRITABLE",
                (false, false) => "READONLY",
            };
            writeln!(f, "  #{:<2} [{}] {}", i, flags, key)?;
        }
        writeln!(f)?;

        writeln!(f, "Recent Blockhash: {}", self.recent_blockhash)?;
        writeln!(f)?;

        writeln!(f, "Instructions ({}):", self.instructions.len())?;
        for ix in &self.instructions {
            write!(f, "{}", ix)?;
        }

        if !self.address_table_lookups.is_empty() {
            writeln!(f)?;
            writeln!(
                f,
                "Address Table Lookups ({}):",
                self.address_table_lookups.len()
            )?;
            for (i, lookup) in self.address_table_lookups.iter().enumerate() {
                writeln!(f, "  [{}] Table: {}", i, lookup.account_key)?;
                writeln!(f, "      Writable indices: {:?}", lookup.writable_indexes)?;
                writeln!(f, "      Readonly indices: {:?}", lookup.readonly_indexes)?;
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for DecodedInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tag = if self.fill.is_some() {
            " [fill_exact_in]"
        } else {
            ""
        };
        writeln!(
            f,
            "  Instruction #{}{}: program={} (index={})",
            self.instruction_index, tag, self.program_id, self.program_id_index,
        )?;
        writeln!(
            f,
            "    Data ({} bytes): {}",
            self.data.len(),
            to_hex(&self.data),
        )?;
        writeln!(f, "    Accounts ({}):", self.accounts.len())?;
        for acct in &self.accounts {
            let flags = match (acct.is_signer, acct.is_writable) {
                (true, true) => "SIGNER WRITABLE",
                (true, false) => "SIGNER READONLY",
                (false, true) => "WRITABLE",
                (false, false) => "READONLY",
            };
            let label = acct
                .label
                .as_deref()
                .map(|l| format!(" ({})", l))
                .unwrap_or_default();
            writeln!(
                f,
                "      #{:<2} [{}] {}{}",
                acct.index, flags, acct.pubkey, label,
            )?;
        }

        if let Some((fill_ix, analysis)) = &self.fill {
            writeln!(f, "    --- Fill Details ---")?;
            writeln!(f, "    Taker side:      {}", fill_ix.taker_side)?;
            writeln!(f, "    Amount in:       {} atoms", fill_ix.amount_in_atoms)?;
            writeln!(f, "    Expire at:       {}", fill_ix.params.expire_at)?;
            writeln!(f, "    Tick size (qpb): {}", fill_ix.params.tick_size_qpb)?;
            writeln!(f, "    Lot size (base): {}", fill_ix.params.lot_size_base)?;
            writeln!(f, "    Levels ({}):", fill_ix.params.levels.len())?;
            for (i, lvl) in fill_ix.params.levels.iter().enumerate() {
                writeln!(
                    f,
                    "      #{}: px_ticks={}, qty_lots={}",
                    i, lvl.px_ticks, lvl.qty_lots,
                )?;
            }
            writeln!(f, "    --- Sweep Analysis ---")?;
            writeln!(
                f,
                "    Amount spent:    {} atoms",
                analysis.amount_spent_atoms
            )?;
            writeln!(
                f,
                "    Amount out:      {} atoms",
                analysis.amount_out_atoms
            )?;
            writeln!(f, "    VWAP (ticks):    {}", analysis.vwap_ticks)?;
            writeln!(f, "    Levels consumed: {}", analysis.levels_consumed)?;
            writeln!(f, "    Total lots:      {}", analysis.total_lots_filled)?;
            writeln!(
                f,
                "    Effective price: {:.6} quote-atoms/base-atom",
                analysis.effective_price()
            )?;
        }

        if let Some(jupiter) = &self.jupiter {
            writeln!(f, "    --- Jupiter Route ---")?;
            writeln!(f, "    Kind:            {}", jupiter.kind)?;
            writeln!(f, "    In amount:       {} atoms", jupiter.in_amount)?;
            writeln!(f, "    Quoted out:      {} atoms", jupiter.quoted_out_amount)?;
            writeln!(f, "    Slippage:        {} bps", jupiter.slippage_bps)?;
            writeln!(f, "    Platform fee:    {} bps", jupiter.platform_fee_bps)?;
            if let Some(ps) = jupiter.positive_slippage_bps {
                writeln!(f, "    Positive slip.:  {} bps", ps)?;
            }
            writeln!(f, "    Hops ({}):", jupiter.hops.len())?;
            for hop in &jupiter.hops {
                writeln!(
                    f,
                    "      #{} {} [{} -> {}] share={}bps in={:?} out={:?}",
                    hop.hop_index,
                    hop.swap,
                    hop.input_index,
                    hop.output_index,
                    hop.share_bps,
                    hop.estimated_in_amount,
                    hop.estimated_out_amount,
                )?;
            }
        }

        Ok(())
    }
}
