use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Debug, Deserialize)]
pub struct CrawlStats {
    pub pages_visited: usize,
    pub links_followed: usize,
    pub links_ignored: usize,
    pub start_time: u64, // Time in milliseconds since UNIX_EPOCH
}

impl CrawlStats {
    pub fn new() -> Self {
        Self {
            pages_visited: 0,
            links_followed: 0,
            links_ignored: 0,
            start_time: current_time_millis(),
        }
    }
}

fn current_time_millis() -> u64 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration.as_millis() as u64
}
