use assert_cmd::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use tempfile::TempDir;

mod common;
use common::{create_test_keystore, test_command, TEST_EVM_KEY as VALID_EVM_KEY};

fn write_empty_config(temp_dir: &TempDir) {
    let purl_dir = temp_dir.path().join(".purl");
    fs::create_dir_all(&purl_dir).expect("Failed to create .purl directory");
    fs::write(purl_dir.join("config.toml"), "networks = []\ntokens = []\n")
        .expect("Failed to write empty config");
}

fn spawn_payment_required_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test server");
    let address = listener.local_addr().expect("Missing test server address");
    let url = format!("http://{}/paid", address);

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("Failed to accept test connection");
        let mut buffer = [0_u8; 8192];
        let _ = stream.read(&mut buffer);

        let body = serde_json::json!({
            "x402Version": 2,
            "error": "Payment Required",
            "accepts": [
                {
                    "scheme": "exact",
                    "network": "eip155:84532",
                    "amount": "10000",
                    "asset": "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
                    "payTo": "0x1111111111111111111111111111111111111111",
                    "maxTimeoutSeconds": 60,
                    "extra": {
                        "name": "USD Coin",
                        "version": "2"
                    }
                }
            ],
            "resource": {
                "url": format!("http://{}/paid", address),
                "description": "Dry run test endpoint",
                "mimeType": "application/json"
            }
        })
        .to_string();

        let encoded = {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(body.as_bytes())
        };

        let response = format!(
            "HTTP/1.1 402 Payment Required\r\nContent-Type: application/json\r\nPAYMENT-REQUIRED: {encoded}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );

        stream
            .write_all(response.as_bytes())
            .expect("Failed to write test response");
    });

    (url, handle)
}

#[test]
#[serial]
fn test_dry_run_accepts_private_key_without_preconfigured_wallet() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    write_empty_config(&temp);
    let (url, handle) = spawn_payment_required_server();

    test_command(&temp)
        .args([
            "--dry-run",
            "--private-key",
            VALID_EVM_KEY,
            "--network",
            "base-sepolia",
            "-X",
            "POST",
            "--json",
            "{\"demo\":true}",
            &url,
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("[DRY RUN] Payment would be made:"))
        .stdout(predicate::str::contains("eip155:84532"))
        .stderr(predicate::str::contains("Dry run completed"))
        .stderr(predicate::str::contains("No wallet configured").not());

    handle.join().expect("Server thread panicked");
}

#[test]
#[serial]
fn test_dry_run_accepts_wallet_path_without_preconfigured_wallet() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    write_empty_config(&temp);
    let keystore_path =
        create_test_keystore(&temp, "override-wallet", VALID_EVM_KEY, "test-password");
    let keystore_path_str = keystore_path.to_string_lossy().into_owned();
    let (url, handle) = spawn_payment_required_server();

    test_command(&temp)
        .args([
            "--dry-run",
            "--wallet",
            &keystore_path_str,
            "--password",
            "test-password",
            "--network",
            "base-sepolia",
            "-X",
            "POST",
            "--json",
            "{\"demo\":true}",
            &url,
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("[DRY RUN] Payment would be made:"))
        .stdout(predicate::str::contains("eip155:84532"))
        .stderr(predicate::str::contains("Dry run completed"))
        .stderr(predicate::str::contains("No wallet configured").not());

    handle.join().expect("Server thread panicked");
}
