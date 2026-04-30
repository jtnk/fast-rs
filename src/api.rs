use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;

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

#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    pub city: String,
    pub country: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Client {
    pub ip: String,
    #[allow(dead_code)]
    pub asn: String,
    pub isp: String,
    #[allow(dead_code)]
    pub location: Location,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    #[allow(dead_code)]
    pub name: String,
    pub url: String,
    pub location: Location,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Targets {
    pub client: Client,
    pub targets: Vec<Target>,
}

const TARGETS_API: &str = "https://api.fast.com";

/// Fetch CDN targets and client metadata from the fast.com API.
pub async fn fetch_targets(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    url_count: u32,
) -> Result<Targets> {
    let url =
        format!("{base_url}/netflix/speedtest/v2?https=true&token={token}&urlCount={url_count}");
    let resp = client.get(&url).send().await?.error_for_status()?;
    Ok(resp.json::<Targets>().await?)
}

/// Convenience wrapper using the production API base URL.
pub async fn fetch_targets_default(
    client: &reqwest::Client,
    token: &str,
    url_count: u32,
) -> Result<Targets> {
    fetch_targets(client, TARGETS_API, token, url_count).await
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

    #[tokio::test]
    async fn fetch_targets_parses_response() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "client": {
                "ip": "203.0.113.7",
                "asn": "AS15169",
                "isp": "TestNet",
                "location": {"city": "Dublin", "country": "Ireland"}
            },
            "targets": [
                {
                    "name": "ipv4-c001-dub001-ix.1.oca.nflxvideo.net",
                    "url": "https://ipv4-c001-dub001-ix.1.oca.nflxvideo.net/speedtest?c=ie&n=15169&v=4&e=1",
                    "location": {"city": "Dublin", "country": "Ireland"}
                }
            ]
        });
        Mock::given(method("GET"))
            .and(path("/netflix/speedtest/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let targets = fetch_targets(&client, &server.uri(), "TOKEN", 5)
            .await
            .unwrap();
        assert_eq!(targets.client.ip, "203.0.113.7");
        assert_eq!(targets.targets.len(), 1);
        assert!(targets.targets[0].url.contains("nflxvideo.net"));
    }
}
