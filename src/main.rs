mod crawler;
mod state;
mod stats;
mod utils;

use crate::crawler::start_crawl;
use crossbeam::queue::SegQueue;
use state::{load_state, save_state};
use stats::CrawlStats;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use anyhow::Result;

fn main() -> Result<()> {
    let start_url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let queue = Arc::new(SegQueue::new());
    let visited = Arc::new(Mutex::new(HashSet::<String>::new()));
    let stats = Arc::new(Mutex::new(CrawlStats::new()));

    // Load crawl state if available
    if let Ok(state) = load_state() {
        for (url, depth) in state.queue {
            queue.push((url, depth));
        }
        let mut visited_guard = visited.lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire visited lock: {}", e))?;
        *visited_guard = state.visited;
    } else {
        queue.push((start_url.to_string(), 0));
    }

    start_crawl(&queue, &visited, &stats)?;

    let visited_pages = visited.lock()
        .map_err(|e| anyhow::anyhow!("Failed to acquire visited lock: {}", e))?;
    println!("Visited pages: {:?}", *visited_pages);
    state::save_visited(&visited_pages)?;

    // Save crawl state
    let state = state::CrawlState {
        queue: {
            let mut queue_vec = vec![];
            while let Some(item) = queue.pop() {
                queue_vec.push(item);
            }
            queue_vec
        },
        visited: visited_pages.clone(),
    };
    save_state(&state)?;

    // Show statistics
    let stats_guard = stats.lock()
        .map_err(|e| anyhow::anyhow!("Failed to acquire stats lock: {}", e))?;
    println!("Crawl statistics: {:?}", *stats_guard);
    
    Ok(())
}
