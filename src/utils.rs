use anyhow::{Result, Context};

pub async fn fetch_page(url: &str) -> Result<String> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to fetch URL: {}", url))?;
    let body = response.text()
        .await
        .with_context(|| format!("Failed to read response body from URL: {}", url))?;
    Ok(body)
}
