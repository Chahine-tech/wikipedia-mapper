use reqwest::Error as ReqwestError;

pub fn fetch_page(url: &str) -> Result<String, ReqwestError> {
    let response = reqwest::blocking::get(url)?;
    let body = response.text()?;
    Ok(body)
}
