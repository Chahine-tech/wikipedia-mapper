use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::collections::HashSet;
use anyhow::{Result, Context};

#[derive(Serialize, Deserialize)]
pub struct CrawlState {
    pub queue: Vec<(String, usize)>, // (URL, depth)
    pub visited: HashSet<String>,
}

pub fn save_state(state: &CrawlState) -> Result<()> {
    let serialized = serde_json::to_string(state)
        .context("Failed to serialize crawl state")?;
    let mut file = File::create("crawl_state.json")
        .context("Failed to create crawl_state.json")?;
    file.write_all(serialized.as_bytes())
        .context("Failed to write crawl state to file")?;
    Ok(())
}

pub fn load_state() -> Result<CrawlState> {
    let file = File::open("crawl_state.json")
        .context("Failed to open crawl_state.json")?;
    let state: CrawlState = serde_json::from_reader(file)
        .context("Failed to deserialize crawl state")?;
    Ok(state)
}

// pub fn save_visited(visited: &HashSet<String>) -> Result<()> {
//     let serialized = serde_json::to_string(visited)
//         .context("Failed to serialize visited pages")?;
//     let mut file = File::create("visited_pages.json")
//         .context("Failed to create visited_pages.json")?;
//     file.write_all(serialized.as_bytes())
//         .context("Failed to write visited pages to file")?;
//     Ok(())
// }
