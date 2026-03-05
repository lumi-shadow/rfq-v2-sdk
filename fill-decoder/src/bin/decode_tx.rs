
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "decode-tx",
    about = "Decode RFQ v2 fill_exact_in transactions from Solana",
    long_about = "Decode and analyse RFQ v2 fill_exact_in instructions embedded \
                  in Solana transactions. Supports both direct base-64 message \
                  decoding and fetching transactions from an RPC node."
)]
struct Cli {
    #[arg(long, group = "input")]
    message_hash: Option<String>,

    #[arg(long, group = "input")]
    tx: Option<String>,

    #[arg(long, env = "RPC_URL")]
    rpc_url: Option<String>,
}

mod rpc {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize)]
    pub struct Request {
        pub jsonrpc: &'static str,
        pub id: u64,
        pub method: &'static str,
        pub params: serde_json::Value,
    }

    #[derive(Deserialize)]
    pub struct Response {
        pub result: Option<TransactionResult>,
        pub error: Option<RpcError>,
    }

    #[derive(Deserialize)]
    pub struct RpcError {
        pub code: i64,
        pub message: String,
    }

    #[derive(Deserialize)]
    pub struct TransactionResult {
        pub transaction: (String, String),
    }
}

async fn fetch_transaction_base64(rpc_url: &str, signature: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let body = rpc::Request {
        jsonrpc: "2.0",
        id: 1,
        method: "getTransaction",
        params: serde_json::json!([
            signature,
            {
                "encoding": "base64",
                "maxSupportedTransactionVersion": 0
            }
        ]),
    };

    let resp = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("RPC returned HTTP {}", resp.status()));
    }

    let rpc_resp: rpc::Response = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse RPC response: {e}"))?;

    if let Some(err) = rpc_resp.error {
        return Err(format!("RPC error ({}): {}", err.code, err.message));
    }

    let result = rpc_resp
        .result
        .ok_or_else(|| "RPC returned null result – transaction not found".to_string())?;

    Ok(result.transaction.0)
}

fn decode_and_print(b64: &str) {
    match fill_decoder::decode_transaction_base64(b64) {
        Ok(tx) => println!("{tx}"),
        Err(e) => {
            eprintln!("Failed to decode transaction: {e}");
            std::process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let b64 = if let Some(b64) = cli.message_hash {
        b64
    } else if let Some(sig) = cli.tx {
        let rpc_url = cli.rpc_url.unwrap_or_else(|| {
            eprintln!("Error: --tx requires an RPC URL. Set RPC_URL env var or pass --rpc-url");
            std::process::exit(1);
        });
        eprintln!("Fetching tx {} from {} …", sig, rpc_url);
        match fetch_transaction_base64(&rpc_url, &sig).await {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to fetch transaction: {e}");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Error: provide either --message-hash or --tx");
        std::process::exit(1);
    };

    decode_and_print(&b64);
}
