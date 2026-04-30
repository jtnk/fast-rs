use anyhow::{Context, Result};
use regex::Regex;

/// Extract the path of the application JS bundle from the fast.com homepage HTML.
pub fn parse_app_js_path(html: &str) -> Result<String> {
    let re = Regex::new(r#"src="(/app-[A-Za-z0-9]+\.js)""#).unwrap();
    let caps = re
        .captures(html)
        .context("no app-*.js script tag found in fast.com HTML")?;
    Ok(caps[1].to_string())
}

/// Extract the API token from the fast.com app JS bundle.
pub fn parse_token(js: &str) -> Result<String> {
    let re = Regex::new(r#"token:"([A-Za-z0-9]+)""#).unwrap();
    let caps = re
        .captures(js)
        .context("no token literal found in fast.com JS bundle")?;
    Ok(caps[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_app_js_path() {
        let html = r#"<html><body><script src="/app-abc123.js"></script></body></html>"#;
        assert_eq!(parse_app_js_path(html).unwrap(), "/app-abc123.js");
    }

    #[test]
    fn extracts_app_js_path_returns_error_when_missing() {
        assert!(parse_app_js_path("<html></html>").is_err());
    }

    #[test]
    fn extracts_token() {
        let js = r#"function n(){return{urlCount:5,token:"YXNkZmFzZGZhc2RmYXNkZg",https:!0}}"#;
        assert_eq!(parse_token(js).unwrap(), "YXNkZmFzZGZhc2RmYXNkZg");
    }

    #[test]
    fn extracts_token_returns_error_when_missing() {
        assert!(parse_token("var x = 1;").is_err());
    }

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_token_walks_homepage_then_js_bundle() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"<script src="/app-xyz.js"></script>"#),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/app-xyz.js"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"...token:"DEADBEEF",..."#))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let token = fetch_token(&client, &server.uri()).await.unwrap();
        assert_eq!(token, "DEADBEEF");
    }
}

const FASTCOM_HOMEPAGE: &str = "https://fast.com";

/// Scrape fast.com homepage + JS bundle, return the API token.
pub async fn fetch_token(client: &reqwest::Client, base_url: &str) -> Result<String> {
    let html = client
        .get(base_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let js_path = parse_app_js_path(&html)?;
    let js_url = format!("{base_url}{js_path}");
    let js = client
        .get(&js_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_token(&js)
}

/// Convenience wrapper that fetches the token using the real fast.com URL.
pub async fn fetch_token_default(client: &reqwest::Client) -> Result<String> {
    fetch_token(client, FASTCOM_HOMEPAGE).await
}
