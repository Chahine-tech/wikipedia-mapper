use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};

#[derive(Serialize, Deserialize)]
pub struct CrawlState {
    pub queue: Vec<(String, usize)>, // (URL, depth)
    pub visited: Vec<String>,
}

pub fn save_state(state: &CrawlState) -> io::Result<()> {
    let serialized = serde_json::to_string(state)?;
    let mut file = File::create("crawl_state.json")?;
    file.write_all(serialized.as_bytes())?;
    Ok(())
}

pub fn load_state() -> io::Result<CrawlState> {
    let file = File::open("crawl_state.json")?;
    let state: CrawlState = serde_json::from_reader(file)?;
    Ok(state)
}

pub fn save_visited(visited: &Vec<String>) -> io::Result<()> {
    let serialized = serde_json::to_string(visited)?;
    let mut file = File::create("visited_pages.json")?;
    file.write_all(serialized.as_bytes())?;
    Ok(())
}
