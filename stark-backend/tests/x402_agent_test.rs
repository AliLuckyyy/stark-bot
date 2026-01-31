//! Integration test for x402_agent_invoke tool

use serde_json::json;

#[tokio::test]
async fn test_x402_agent_health() {
    // Test the health endpoint (free, no payment required)
    let client = reqwest::Client::new();

    let response = client
        .post("https://dad-jokes-agent-production.up.railway.app/entrypoints/health/invoke")
        .header("Content-Type", "application/json")
        .json(&json!({"input": {}}))
        .send()
        .await
        .expect("Request should succeed");

    assert!(response.status().is_success(), "Health check should succeed");

    let body: serde_json::Value = response.json().await.expect("Should parse JSON");
    assert_eq!(body["status"], "succeeded");
    println!("Health check passed: {:?}", body);
}

#[tokio::test]
async fn test_x402_agent_402_response() {
    // Test that joke endpoint returns 402 without payment
    let client = reqwest::Client::new();

    let response = client
        .post("https://dad-jokes-agent-production.up.railway.app/entrypoints/joke/invoke")
        .header("Content-Type", "application/json")
        .json(&json!({"input": {}}))
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(response.status().as_u16(), 402, "Should return 402 Payment Required");

    let body: serde_json::Value = response.json().await.expect("Should parse JSON");
    assert!(body["accepts"].is_array(), "Should have accepts array");
    assert!(body["accepts"][0]["payTo"].is_string(), "Should have payTo address");
    assert!(body["accepts"][0]["maxAmountRequired"].is_string(), "Should have amount");

    println!("402 response format verified: network={}, amount={}, payTo={}",
        body["accepts"][0]["network"],
        body["accepts"][0]["maxAmountRequired"],
        body["accepts"][0]["payTo"]
    );
}
