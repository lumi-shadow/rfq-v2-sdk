pub mod analysis;
pub mod decode;
pub mod error;
pub mod jupiter;
pub mod rfq_v2;
pub mod transaction;
pub mod types;

pub use error::{FillDecoderError, Result};

pub use types::{
    FillAccounts, FillAnalysis, FillExactInInstruction, FillExactInParams, Level, Side,
};

pub use decode::{
    decode_fill_accounts, decode_fill_accounts_bytes, decode_fill_instruction, decode_fill_params,
    fill_exact_in_discriminator, is_fill_exact_in, FILL_ACCOUNT_LABELS, RFQ_V2_PROGRAM_ID,
};

pub use analysis::analyze_fill;

pub use transaction::{
    decode_message_base64, decode_mm_tx_base64, decode_mm_tx_base64_json,
    decode_transaction_base64, decode_transaction_bytes, filter_mm_summary,
    mm_summary_from_decoded_tx, AddressTableLookup, DecodedInstruction, DecodedJupiterHop,
    DecodedJupiterInstruction, DecodedMessage, DecodedTransaction, MessageHeader, MessageVersion,
    MmFillSummary, MmInstructionSummary, MmJupiterHopSummary, MmJupiterSummary, MmTransferSummary,
    MmTxSummary, ResolvedAccount,
};

pub use jupiter::{
    is_jupiter_route, route_discriminator, route_v2_discriminator,
    shared_accounts_route_discriminator, shared_accounts_route_v2_discriminator,
    DecodedJupiterRoute, RoutePlanStep, RoutePlanStepV2, Side as JupiterSide, Swap,
    JUPITER_PROGRAM_ID,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn build_instruction_data(
        side: Side,
        amount_in: u64,
        expire_at: u64,
        tick_size_qpb: u64,
        lot_size_base: u64,
        levels: &[(u64, u64)],
    ) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(fill_exact_in_discriminator());
        data.push(side as u8);
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&expire_at.to_le_bytes());
        data.extend_from_slice(&tick_size_qpb.to_le_bytes());
        data.extend_from_slice(&lot_size_base.to_le_bytes());
        data.extend_from_slice(&(levels.len() as u32).to_le_bytes());
        for (px, qty) in levels {
            data.extend_from_slice(&px.to_le_bytes());
            data.extend_from_slice(&qty.to_le_bytes());
        }
        data
    }

    #[test]
    fn test_discriminator_check() {
        let data = build_instruction_data(Side::Bid, 0, 0, 1, 1, &[]);
        assert!(is_fill_exact_in(&data));
        assert!(!is_fill_exact_in(&[0u8; 8]));
        assert!(!is_fill_exact_in(&[0u8; 4]));
    }

    #[test]
    fn test_decode_roundtrip() {
        let levels = vec![(100, 50), (105, 30)];
        let data = build_instruction_data(Side::Ask, 1_000_000, 999, 1_000, 1_000_000, &levels);

        let ix = decode_fill_instruction(&data).unwrap();
        assert!(matches!(ix.taker_side, Side::Ask));
        assert_eq!(ix.amount_in_atoms, 1_000_000);
        assert_eq!(ix.params.expire_at, 999);
        assert_eq!(ix.params.tick_size_qpb, 1_000);
        assert_eq!(ix.params.lot_size_base, 1_000_000);
        assert_eq!(ix.params.levels.len(), 2);
        assert_eq!(ix.params.levels[0].px_ticks, 100);
        assert_eq!(ix.params.levels[0].qty_lots, 50);
        assert_eq!(ix.params.levels[1].px_ticks, 105);
        assert_eq!(ix.params.levels[1].qty_lots, 30);
    }

    #[test]
    fn test_decode_accounts() {
        use solana_sdk::pubkey::Pubkey;
        let keys: Vec<Pubkey> = (0..11)
            .map(|i| Pubkey::new_from_array([i as u8; 32]))
            .collect();
        let accs = decode_fill_accounts(&keys).unwrap();
        assert_eq!(accs.user, Pubkey::new_from_array([0u8; 32]));
        assert_eq!(accs.fill_authority, Pubkey::new_from_array([1u8; 32]));
        assert_eq!(
            accs.maker_base_token_account,
            Pubkey::new_from_array([4u8; 32])
        );
        assert_eq!(accs.quote_mint, Pubkey::new_from_array([7u8; 32]));
    }

    #[test]
    fn test_decode_accounts_bytes() {
        let keys: Vec<[u8; 32]> = (0..11).map(|i| [i as u8; 32]).collect();
        let accs = decode_fill_accounts_bytes(&keys).unwrap();
        assert_eq!(
            accs.user,
            solana_sdk::pubkey::Pubkey::new_from_array([0u8; 32])
        );
        assert_eq!(
            accs.fill_authority,
            solana_sdk::pubkey::Pubkey::new_from_array([1u8; 32])
        );
    }

    #[test]
    fn test_decode_accounts_too_few() {
        use solana_sdk::pubkey::Pubkey;
        let keys: Vec<Pubkey> = (0..5)
            .map(|i| Pubkey::new_from_array([i as u8; 32]))
            .collect();
        assert!(decode_fill_accounts(&keys).is_err());
    }

    #[test]
    fn test_analyze_bid_single_level() {
        let data = build_instruction_data(
            Side::Bid,
            500,  // 500 quote-atoms in
            9999, // expire_at
            1,    // tick_size_qpb
            1,    // lot_size_base (raw)
            &[(100, 10)],
        );
        let ix = decode_fill_instruction(&data).unwrap();
        let analysis = analyze_fill(&ix).unwrap();

        assert!(matches!(analysis.taker_side, Side::Bid));
        assert_eq!(analysis.amount_spent_atoms, 500);
        assert_eq!(analysis.amount_out_atoms, 5);
        assert_eq!(analysis.total_lots_filled, 5);
        assert_eq!(analysis.vwap_ticks, 100);
        assert_eq!(analysis.levels_consumed, 1);
    }

    #[test]
    fn test_analyze_bid_multi_level() {
        let data = build_instruction_data(
            Side::Bid,
            1000,
            9999,
            1, // tick_size_qpb
            1, // lot_size_base
            &[(100, 5), (200, 5)],
        );
        let ix = decode_fill_instruction(&data).unwrap();
        let analysis = analyze_fill(&ix).unwrap();

        assert_eq!(analysis.amount_spent_atoms, 900);
        assert_eq!(analysis.amount_out_atoms, 7);
        assert_eq!(analysis.total_lots_filled, 7);
        assert_eq!(analysis.levels_consumed, 2);
        assert_eq!(analysis.vwap_ticks, 128);
    }

    #[test]
    fn test_analyze_ask_single_level() {
        let data = build_instruction_data(
            Side::Ask,
            10,   // 10 base-atoms in
            9999, // expire_at
            1,    // tick_size_qpb
            2,    // lot_size_base
            &[(50, 10)],
        );
        let ix = decode_fill_instruction(&data).unwrap();
        let analysis = analyze_fill(&ix).unwrap();

        assert!(matches!(analysis.taker_side, Side::Ask));
        assert_eq!(analysis.amount_spent_atoms, 10);
        assert_eq!(analysis.amount_out_atoms, 250);
        assert_eq!(analysis.total_lots_filled, 5);
        assert_eq!(analysis.vwap_ticks, 50);
    }

    #[test]
    fn test_effective_price_bid() {
        let data = build_instruction_data(Side::Bid, 500, 9999, 1, 1, &[(100, 10)]);
        let ix = decode_fill_instruction(&data).unwrap();
        let analysis = analyze_fill(&ix).unwrap();

        assert!((analysis.effective_price() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_display() {
        let data = build_instruction_data(Side::Bid, 500, 9999, 1, 1, &[(100, 10)]);
        let ix = decode_fill_instruction(&data).unwrap();
        let analysis = analyze_fill(&ix).unwrap();
        let s = format!("{}", analysis);
        assert!(s.contains("Bid"));
        assert!(s.contains("vwap_ticks: 100"));
    }

    const REAL_TX_BASE64: &str = "AsPqw9SAB7rMKDuWgFVxTnfagAj/mSIwuKrYVM3csciSD2HOcJfht8nYL9sARghcVsJlxtTT0uaudrmCDEV1PwkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAIBBg7KTUteDZUF9eqHCzJvdWHUARq8NQU4DIdyCSngydvcb3bk5le5izsp4223iXZnE4grlusUmzXQL4KHUihpkIa4ZNs5dmgwxMyL/IfP1O5Ac9iAQLnbqGpcsdM0PwusuV3sGjl7eLbP6JfIktH4I1aEAyciR2HwKu4QXwEGaOhOz2Hiyz7h80+tj7g8An6p3AGnu96N6DCinehLp7TnorlL9sMrLW9QqpZr3Vb5mV0mnsAM1mxk/2i/SD2e+0t5s5TXDN1sPYOMmaxV5QajzizK3Ud8JdMKkPML/GYipWTOIZRVvlKNhT2MVZBGmyT2HRRgChORTrPsbIfA4a0nvrlLAwZGb+UhFzL/7K26csOb57yM5bvF9xJrLEObOkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAR51VvyMcBu7nTFbs5oFQf9sbLeo/SOUQKxzaJWvBOPBt324ddloZPZy+FGzut5rBy0he1fWzeROoz1hX7/AKmMlyWPTiSJ8bs9ECkUjg2DC1oTmdr/EIQEjnvY2+n4WQnk1I8BnjipjWjfEIVY5ELgMj6qznalm0dryLHTf7MuPFxhbWYWCgjj9eAH/Sz/1aYw9GQui88WwYxH3hd9dvYHCAAFAtmOAwAIAAkDzPEfAAAAAAAJAgACDAIAAADwSlABAAAAAAoFAgAZCwkJk/F7ZPSErnb/DAYAAwAdCQsBAQo0AAIDGR0LCwoaCg4NAAECBAUGGRsLCyAhACIWBwQXGBcYGAsgCh4AHw8HAxAREgsTFBUKHGO7ZPrMMcSvFAAtMQEAAAAAYHrfdQAAAABkAAoAAAADAAAAeAEsAAAAqG+UaQAAAAABAAAAAAAAAOgDAAAAAAAAAQAAAGMAAAAAAAAAQEIPAAAAAAAQJwABaAAQJwECGhAnAgMLAwIAAAEJAym/lQcqT78E33F1k+c4vMwhJygVwkcagNn59VWw1IQlASQEFwAoAV0T1DATNHXpX/sFnj+G3qAzHNFlFcd3JmJW5UXLSEmBB7Swr6yxXbIDrlmz4k8+KpZXHoZ2stouSFQQDE0nTzzoyEvg3OKGj8kaqe0DAQYEAwMCAA==";

    #[test]
    fn test_decode_real_transaction() {
        let tx = decode_transaction_base64(REAL_TX_BASE64).unwrap();

        assert_eq!(tx.signatures.len(), 2);
        assert!(tx.signatures[0].starts_with("4vBpXi9zG"));

        let msg = &tx.message;
        assert_eq!(msg.version, MessageVersion::V0);
        assert_eq!(msg.header.num_required_signatures, 2);
        assert_eq!(msg.header.num_readonly_signed_accounts, 1);
        assert_eq!(msg.header.num_readonly_unsigned_accounts, 6);
        assert_eq!(msg.account_keys.len(), 14);

        assert!(msg.instructions.len() >= 2);

        let fill_ix = msg
            .instructions
            .iter()
            .find(|ix| ix.fill.is_some())
            .expect("should find embedded fill_exact_in params in a Jupiter instruction");

        let (fill, analysis) = fill_ix.fill.as_ref().unwrap();
        let jupiter = fill_ix
            .jupiter
            .as_ref()
            .expect("should decode Jupiter route details");

        assert!(fill.amount_in_atoms > 0);
        assert!(fill.params.tick_size_qpb > 0);
        assert!(fill.params.lot_size_base > 0);
        assert!(!fill.params.levels.is_empty());
        assert!(!jupiter.hops.is_empty());
        assert_eq!(jupiter.in_amount, fill.amount_in_atoms);

        assert!(analysis.amount_out_atoms > 0);
        assert!(analysis.vwap_ticks > 0);
        assert!(analysis.levels_consumed > 0);
    }

    #[test]
    fn test_decode_real_message_only() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let tx_bytes = STANDARD.decode(REAL_TX_BASE64).unwrap();
        let msg_bytes = &tx_bytes[129..];
        let msg_b64 = STANDARD.encode(msg_bytes);

        let msg = decode_message_base64(&msg_b64).unwrap();

        assert_eq!(msg.version, MessageVersion::V0);
        assert_eq!(msg.account_keys.len(), 14);

        let fill_ix = msg
            .instructions
            .iter()
            .find(|ix| ix.fill.is_some())
            .expect("should find embedded fill_exact_in params");

        let (fill, analysis) = fill_ix.fill.as_ref().unwrap();
        assert!(fill.amount_in_atoms > 0);
        assert!(fill.params.tick_size_qpb > 0);
        assert!(analysis.levels_consumed > 0);
    }

    const REAL_TX2_BASE64: &str = "AlPlBOM0/PJtMdGe0Umk2ZL+l83VLIZlnto+clZr77+LZpMHjFOMhyn8d9paVwW2MUB5yfiVF9rQoqDX+HdVZgQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAIBBQuWmpufqjVYZfnyuFyDIYJfGWS2mAfjxKCUSfgP6ocw/3bk5le5izsp4223iXZnE4grlusUmzXQL4KHUihpkIa4FRxK4rsywvjpGrtIXvcwlTqIfhWBjuSSKYumfXogybHMphZItU08AEZ6ahGHYgLn6pzxnUhwhDZJYTR1tfeVfdtjecj/osn4Wsppt6OzLN+qBRRBwfnZU+16g1g0LNZ01wzdbD2DjJmsVeUGo84syt1HfCXTCpDzC/xmIqVkziGGVoqJoXHQv3NrqnA8ud4Fpk5exDk5GSvBfPxu23BzewMGRm/lIRcy/+ytunLDm+e8jOW7xfcSayxDmzpAAAAACeTUjwGeOKmNaN8QhVjkQuAyPqrOdqWbR2vIsdN/sy4EedVb8jHAbu50xW7OaBUH/bGy3qP0jlECsc2iVrwTjwan1RcYe9FmNdrUBFX9wsDBJMaPIVZ1pdu6y18IAAAASNIEZ4DY6P0pwzwN89l81A/PZUIvzSKvmhMEoq6GJccDBwAFAjrVAAAHAAkDQCsAAAAAAAAJFwACAwYMDg4JCwkIAAECAwQFBgwODgoNWLtk+swxxK8UAAk9AAAAAADYNAwAAAAAADIAAAAAAAEAAAB4ASwAAACVbp1pAAAAAAEAAAAAAAAAQEIPAAAAAAABAAAANg0DAAAAAADoAwAAAAAAABAnAAEBKb+VBypPvwTfcXWT5zi8zCEnKBXCRxqA2fn1VbDUhCUABAAoAhQ=";
    const REAL_TX3_BASE64: &str = "Aslm78DCuteQgwSzZ9j2Pz7CZ+rl5/7cw15I4Kq2yt0JNZ3rURrQiFEEsstMxeZA/Zw/i+Fz8T5sYbW085kG4wJdGOb3x/2RzmSmNd1E3sgN729d2H8OES0ATYpJp3nz/c7W6L+ohWSu2gPH2BfXP5DTUjp3D57H7FgqqZMQRMQAgAIBBg2WmpufqjVYZfnyuFyDIYJfGWS2mAfjxKCUSfgP6ocw/3bk5le5izsp4223iXZnE4grlusUmzXQL4KHUihpkIa4FRxK4rsywvjpGrtIXvcwlTqIfhWBjuSSKYumfXogybFgn+qhJ+fWYd3W06tzn+P1czERR6OxLQGVW8rTfcMCxMymFki1TTwARnpqEYdiAufqnPGdSHCENklhNHW195V922N5yP+iyfhaymm3o7Ms36oFFEHB+dlT7XqDWDQs1nTXDN1sPYOMmaxV5QajzizK3Ud8JdMKkPML/GYipWTOIYZWiomhcdC/c2uqcDy53gWmTl7EOTkZK8F8/G7bcHN7AwZGb+UhFzL/7K26csOb57yM5bvF9xJrLEObOkAAAAAJ5NSPAZ44qY1o3xCFWORC4DI+qs52pZtHa8ix03+zLu4D4AEDC1EyOPVN63O+10XapZhJyAkKgg01YuBx11pTBHnVW/IxwG7udMVuzmgVB/2xst6j9I5RArHNola8E48G3fbh12Whk9nL4UbO63msHLSF7V9bN5E6jPWFfv8AqUanJdqjKmUhVdiEjzTylqwcaq0Xna7heHwH9+xlPv1nBQgABQKWNwIACAAJA1F4AwAAAAAACwUDABQMEAmT8Xtk9ISudv8LJwACAwcUDAwLEQsJAAECBAUGBxIMDBcVAA0PDgMEGAwMFxQSChYLE2a7ZPrMMcSvFAA2bgEAAAAA5iF3AwAAAAAyAAAAAAACAAAAeAEsAAAAF86eaQAAAAABAAAAAAAAAEBCDwAAAAAAAQAAAMwNAwAAAAAA6AMAAAAAAAAQJwABduEcpEPYf8pMABAnAQIMAwMAAAEJAim/lQcqT78E33F1k+c4vMwhJygVwkcagNn59VWw1IQlAAUTACgCF3xH91r6if+mzAzbSOvXRJeK+ITK6QB3N1rUPikkrErDA6qlrASoo6ap";
    const REAL_TX4_BASE64: &str = "AiUnxlGxt8kM8bwvrqtBEP0EuTEmAodB+s+GIWBGTFoLzDNN9PQx2vlGexYXblT4floi2GE2MEH6PusQQWyeDwtB9NSoG5JgwPT8Xk01HZBCPvUOiuCB1aNPfpzF/16gVmZZjxIQNutYEER1vcUb2KGZkT3pSbyO4K6iqxK0vfILgAIBBhAJY8tsTvZ/i02dppe1Q9zPHRP3mx2hKZAZFTuEjIe023bk5le5izsp4223iXZnE4grlusUmzXQL4KHUihpkIa4FXpSwvS2eRAGbOSKH6inTi/epO2gaU3+brDqFJiR+stHbw5RuN479O8wEsxh5Cd2HYPsvpMLCO8Nccu4rhO7alNwOgGVhWYlecKbbmYl4tRDmkhWFM+iBn4GVglESqjVX9jKnOK46hwREdAVH65SfJGeGhh3DNKwpeW/ZZ/yWM+wouKaVmSxV6J64pAyAdcslY0k1cZxHvyE0iKIZeqEWNyWajqAbr1TOzBfwah3LsylOQ3LIyGJ75eLgblVnXP91wzdbD2DjJmsVeUGo84syt1HfCXTCpDzC/xmIqVkziH2wystb1CqlmvdVvmZXSaewAzWbGT/aL9IPZ77S3mzlAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwZGb+UhFzL/7K26csOb57yM5bvF9xJrLEObOkAAAAAJ5NSPAZ44qY1o3xCFWORC4DI+qs52pZtHa8ix03+zLgR51VvyMcBu7nTFbs5oFQf9sbLeo/SOUQKxzaJWvBOPBqfVFxh70WY12tQEVf3CwMEkxo8hVnWl27rLXwgAAAAG3fbh12Whk9nL4UbO63msHLSF7V9bN5E6jPWFfv8AqWw71Y0XxJfRVIh+sgrTM2AwMUYULZdSTM3H7Fs4HaurBgsABQJZFgIACwAJA9kUAAAAAAAACgIABwwCAAAAcLS3AAAAAAANBQcAFg8KCZPxe2T0hK52/Q0jAAcDFhgPDw0TDQwAAQcFCQgWFA8PDhkPABADEQUSAgQGFxVeu2T6zDHErxSAlpgAAAAAAJTcUwAAAAAAMgAAAAAAAgAAAHgBLAAAAGb6n2kAAAAAAQAAAAAAAACAlpgAAAAAAAEAAABMXA0AAAAAAGQAAAAAAAAAECcAAREAECcBAg8DBwAAAQkCKb+VBypPvwTfcXWT5zi8zCEnKBXCRxqA2fn1VbDUhCUABAAoAReIEcCyRB5p391xfuFTruPAxbiWr5eGfeW6RVfAdTsy3QOVy5kDycoJ";

    #[test]
    fn test_decode_real_transaction_2() {
        let tx = decode_transaction_base64(REAL_TX2_BASE64).unwrap();

        assert_eq!(tx.signatures.len(), 2);

        let msg = &tx.message;
        assert_eq!(msg.version, MessageVersion::V0);
        assert_eq!(msg.header.num_required_signatures, 2);
        assert_eq!(msg.header.num_readonly_signed_accounts, 1);
        assert_eq!(msg.header.num_readonly_unsigned_accounts, 5);
        assert_eq!(msg.account_keys.len(), 11);

        assert_eq!(msg.instructions.len(), 3);

        let fill_ix = msg
            .instructions
            .iter()
            .find(|ix| ix.fill.is_some())
            .expect("should find embedded fill_exact_in in Jupiter route instruction");

        let (fill, analysis) = fill_ix.fill.as_ref().unwrap();
        let jupiter = fill_ix
            .jupiter
            .as_ref()
            .expect("should decode Jupiter route details");

        assert!(matches!(fill.taker_side, Side::Ask));
        assert_eq!(fill.amount_in_atoms, 4_000_000);
        assert_eq!(fill.params.tick_size_qpb, 1);
        assert_eq!(fill.params.lot_size_base, 1_000_000);
        assert_eq!(fill.params.levels.len(), 1);
        assert_eq!(fill.params.levels[0].px_ticks, 199_990);
        assert_eq!(fill.params.levels[0].qty_lots, 1_000);

        assert!(matches!(analysis.taker_side, Side::Ask));
        assert_eq!(analysis.amount_in_atoms, 4_000_000);
        assert_eq!(analysis.amount_spent_atoms, 4_000_000);
        assert_eq!(analysis.amount_out_atoms, 799_960);
        assert_eq!(analysis.vwap_ticks, 199_990);
        assert_eq!(analysis.levels_consumed, 1);
        assert_eq!(analysis.total_lots_filled, 4);
        assert_eq!(jupiter.in_amount, 4_000_000);
        assert_eq!(jupiter.hops.len(), 1);
        assert_eq!(jupiter.hops[0].estimated_in_amount, Some(4_000_000));
        assert_eq!(jupiter.hops[0].estimated_out_amount, Some(799_960));
        assert_eq!(jupiter.hops[0].swap, "JupiterRfqV2");
    }

    #[test]
    fn test_decode_real_transaction_3_user_base64() {
        let tx = decode_transaction_base64(REAL_TX3_BASE64).unwrap();

        assert_eq!(tx.signatures.len(), 2);
        assert!(matches!(tx.message.version, MessageVersion::V0));
        assert!(tx.message.instructions.len() >= 3);

        let jupiter_ix = tx
            .message
            .instructions
            .iter()
            .find(|ix| ix.jupiter.is_some())
            .expect("should find a Jupiter route instruction");
        let jupiter = jupiter_ix.jupiter.as_ref().unwrap();
        assert!(!jupiter.hops.is_empty());

        let fill_ix = tx
            .message
            .instructions
            .iter()
            .find(|ix| ix.fill.is_some())
            .expect("should find embedded fill in Jupiter route");
        let (fill, analysis) = fill_ix.fill.as_ref().unwrap();
        assert!(fill.amount_in_atoms > 0);
        assert!(analysis.amount_out_atoms > 0);
        assert_eq!(jupiter.in_amount, fill.amount_in_atoms);

        let mm_json = decode_mm_tx_base64_json(REAL_TX3_BASE64).unwrap();
        assert!(!mm_json.is_empty());
    }

    #[test]
    fn test_decode_mm_summary_real_transaction_2() {
        let summary = decode_mm_tx_base64(REAL_TX2_BASE64).unwrap();
        let summary_filtered = decode_mm_tx_base64(REAL_TX2_BASE64).unwrap();
        assert_eq!(summary_filtered.instructions.len(), 1);
        let summary_json = decode_mm_tx_base64_json(REAL_TX2_BASE64).unwrap();
        assert!(!summary_json.is_empty());

        assert_eq!(summary.signatures.len(), 2);
        assert_eq!(summary.instructions.len(), 1);

        let ix = summary
            .instructions
            .iter()
            .find(|ix| ix.jupiter.is_some())
            .expect("should have Jupiter route instruction");

        let j = ix.jupiter.as_ref().unwrap();
        assert_eq!(j.kind, "route_v2");
        assert_eq!(j.in_amount, 4_000_000);
        assert_eq!(j.quoted_out_amount, 799_960);
        assert_eq!(j.hops.len(), 1);
        assert_eq!(j.hops[0].swap, "JupiterRfqV2");
        assert_eq!(j.hops[0].estimated_in_amount, Some(4_000_000));
        assert_eq!(j.hops[0].estimated_out_amount, Some(799_960));

        let fill = ix.fill.as_ref().unwrap();
        assert_eq!(fill.amount_in_atoms, 4_000_000);
        assert_eq!(fill.amount_out_atoms, 799_960);
        assert_eq!(fill.vwap_ticks, 199_990);
    }

    #[test]
    fn test_decode_real_transaction_4_user_base64() {
        let tx = decode_transaction_base64(REAL_TX4_BASE64).unwrap();
        assert_eq!(tx.signatures.len(), 2);
        assert!(matches!(tx.message.version, MessageVersion::V0));

        let summary = decode_mm_tx_base64(REAL_TX4_BASE64).unwrap();
        assert!(!summary.instructions.is_empty());
        assert!(summary
            .instructions
            .iter()
            .any(|ix| ix.fill.is_some() || ix.jupiter.is_some() || ix.transfer.is_some()));

        let summary_json = decode_mm_tx_base64_json(REAL_TX4_BASE64).unwrap();
        assert!(!summary_json.is_empty());
    }

    #[test]
    fn test_decode_rfq_v2_instruction_directly() {
        let ix_data_b58 =
            "41H7hSdH41CwNK41knAqYXEUvho3dCxHRMh9zcVa4vt23gTCXceZTAhYcDZGAW31w7vQ7ZAZ1ussfocqsKro";
        let ix_data = bs58::decode(ix_data_b58).into_vec().unwrap();

        assert!(is_fill_exact_in(&ix_data));

        let fill = decode_fill_instruction(&ix_data).unwrap();

        assert!(matches!(fill.taker_side, Side::Ask));
        assert_eq!(fill.amount_in_atoms, 24_000_000);

        let analysis = analyze_fill(&fill).unwrap();

        assert_eq!(analysis.amount_out_atoms, 4_803_360);
        assert_eq!(analysis.vwap_ticks, 200_140);
        assert_eq!(analysis.levels_consumed, 1);

        assert!(!fill.params.levels.is_empty());
        assert_eq!(fill.params.levels[0].px_ticks, 200_140);
    }
}
