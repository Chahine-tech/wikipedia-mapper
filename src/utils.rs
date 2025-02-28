use anyhow::{Result, Context};

pub async fn fetch_page(url: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let response = client.get(url)
        .header(reqwest::header::ACCEPT_ENCODING, "gzip, deflate")
        .send()
        .await
        .with_context(|| format!("Failed to fetch URL: {}", url))?;
    
    let body = response.text()
        .await
        .with_context(|| format!("Failed to read response body from URL: {}", url))?;
    
    Ok(body)
}
