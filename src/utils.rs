use anyhow::{Context, Result};

pub async fn fetch_page(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; WikipediaMapper/1.0; +https://github.com/yourusername/wikipedia-mapper)")
        .build()?;

    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "text/html")
        .send()
        .await
        .with_context(|| format!("Failed to fetch URL: {}", url))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "HTTP request failed with status {}: {}",
            response.status(),
            url
        ));
    }

    let body = response
        .text()
        .await
        .with_context(|| format!("Failed to read response body from URL: {}", url))?;

    Ok(body)
}
