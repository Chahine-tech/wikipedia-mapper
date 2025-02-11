use anyhow::{Result, Context};

pub fn fetch_page(url: &str) -> Result<String> {
    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to fetch URL: {}", url))?;
    let body = response.text()
        .with_context(|| format!("Failed to read response body from URL: {}", url))?;
    Ok(body)
}
