//! MPP (Machine Payment Protocol) payment handling.
//!
//! Uses the `mpp` crate for protocol parsing and Tempo transaction signing,
//! while using purl's own HTTP client for making requests.

use alloy::primitives::B256;
use alloy::providers::Provider;
use anyhow::{Context, Result};
use mpp::client::{PaymentProvider, TempoNetwork, TempoProvider};
use mpp::{format_authorization, parse_www_authenticate, ProviderBuilder};
use purl_lib::{Config, HttpResponse};

use crate::request::RequestContext;

/// Default Tempo Moderato testnet RPC URL.
const DEFAULT_TEMPO_RPC: &str = "https://rpc.moderato.tempo.xyz";

/// Handle an MPP 402 response.
///
/// 1. Parse the WWW-Authenticate challenge from the initial 402 response
/// 2. Use TempoProvider to sign and create a credential
/// 3. Retry the request with the Authorization header
pub async fn handle_mpp_request(
    config: &Config,
    request_ctx: &RequestContext,
    url: &str,
) -> Result<HttpResponse> {
    let signer = config
        .load_evm_signer()
        .context("EVM wallet required for MPP/Tempo payments. Run 'purl wallet add' to create one.")?;

    if request_ctx.cli.is_verbose() && request_ctx.cli.should_show_output() {
        eprintln!("MPP: using wallet {:#x}", signer.address());
    }

    // Determine Tempo RPC URL from config or default
    let rpc_url = config
        .rpc
        .get("tempo")
        .cloned()
        .unwrap_or_else(|| DEFAULT_TEMPO_RPC.to_string());

    let provider = TempoProvider::new(signer.clone(), &rpc_url)
        .map_err(|e| anyhow::anyhow!("Failed to create Tempo provider: {e}"))?;

    // Auto-fund via Tempo testnet faucet if requested
    if request_ctx.cli.fund {
        eprintln!("Funding {:#x} via Tempo faucet...", signer.address());
        let rpc_provider = ProviderBuilder::new_with_network::<TempoNetwork>()
            .connect_http(rpc_url.parse().context("Invalid Tempo RPC URL")?);
        let result: Result<Vec<B256>, _> = rpc_provider
            .raw_request("tempo_fundAddress".into(), (signer.address(),))
            .await;
        match result {
            Ok(_) => {
                eprintln!("Funded. Waiting for confirmation...");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            Err(e) => eprintln!("Faucet warning: {e}"),
        }
    }

    // Make the initial request to get the 402 challenge
    let response = request_ctx.execute(url, None).await?;

    let www_auth = response
        .get_header("www-authenticate")
        .ok_or_else(|| anyhow::anyhow!("402 response missing WWW-Authenticate header"))?;

    let challenge = parse_www_authenticate(www_auth)
        .map_err(|e| anyhow::anyhow!("Failed to parse MPP challenge: {e}"))?;

    if request_ctx.cli.is_verbose() && request_ctx.cli.should_show_output() {
        eprintln!("MPP: challenge received");
        eprintln!("  Method: {}", challenge.method);
        eprintln!("  Intent: {}", challenge.intent);
        eprintln!("  Realm: {}", challenge.realm);
        eprintln!("  ID: {}", challenge.id);
        if let Some(ref expires) = challenge.expires {
            eprintln!("  Expires: {}", expires);
        }
    }

    if !provider.supports(&challenge.method, &challenge.intent) {
        anyhow::bail!(
            "No provider for MPP method={}, intent={}. Currently supported: tempo/charge",
            challenge.method,
            challenge.intent
        );
    }

    if request_ctx.cli.dry_run {
        if let Ok(request) = mpp::request_from_challenge(&challenge) {
            eprintln!("[DRY RUN] MPP Payment would be made:");
            eprintln!("  Method: {}", challenge.method);
            eprintln!("  Intent: {}", challenge.intent);
            if let Some(amount) = request.get("amount").and_then(|v| v.as_str()) {
                eprintln!("  Amount: {}", amount);
            }
            if let Some(currency) = request.get("currency").and_then(|v| v.as_str()) {
                eprintln!("  Currency: {}", currency);
            }
            if let Some(recipient) = request.get("recipient").and_then(|v| v.as_str()) {
                eprintln!("  Recipient: {}", recipient);
            }
        }
        anyhow::bail!("Dry run completed");
    }

    if request_ctx.cli.is_verbose() && request_ctx.cli.should_show_output() {
        eprintln!("MPP: signing payment...");
    }

    // Use the mpp crate's TempoProvider to sign and create the credential
    let credential = provider
        .pay(&challenge)
        .await
        .map_err(|e| anyhow::anyhow!("MPP payment failed: {e}"))?;

    let auth_header = format_authorization(&credential)
        .map_err(|e| anyhow::anyhow!("Failed to format MPP credential: {e}"))?;

    if request_ctx.cli.is_verbose() && request_ctx.cli.should_show_output() {
        eprintln!("MPP: credential created, retrying request...");
    }

    // Retry the request with the Authorization header
    let headers = vec![("Authorization".to_string(), auth_header)];
    let response = request_ctx.execute(url, Some(&headers)).await?;

    if request_ctx.cli.is_verbose() && request_ctx.cli.should_show_output() {
        eprintln!("MPP: response status {}", response.status_code);

        // Display payment receipt if present
        if let Some(receipt_hdr) = response.get_header("payment-receipt") {
            if let Ok(receipt) = mpp::parse_receipt(receipt_hdr) {
                eprintln!("MPP: payment settled");
                eprintln!("  Reference: {}", receipt.reference);
                eprintln!("  Method: {}", receipt.method);
                eprintln!("  Status: {}", receipt.status);
            }
        }
    }

    // If still 402, report the failure
    if response.is_payment_required() {
        anyhow::bail!("MPP payment was not accepted by the server");
    }

    Ok(response)
}
