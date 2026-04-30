//! Hits real fast.com. Run with `cargo test -- --ignored`.

#[tokio::test]
#[ignore]
async fn full_run_against_real_fastcom() {
    let client = reqwest::Client::builder()
        .user_agent("fastrs/0.1")
        .build()
        .unwrap();
    let token = fastrs::api::fetch_token_default(&client).await.unwrap();
    assert!(!token.is_empty());
    let targets = fastrs::api::fetch_targets_default(&client, &token, 3)
        .await
        .unwrap();
    assert!(!targets.targets.is_empty());

    let report = fastrs::measure::run(
        &client,
        &targets,
        &fastrs::measure::Options { no_upload: true },
    )
    .await
    .unwrap();
    assert!(report.download_mbps > 0.1);
}
