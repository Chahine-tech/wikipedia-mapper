mod crawler;
mod state;
mod stats;
mod utils;

use crate::crawler::start_crawl;
use crossbeam::queue::SegQueue;
use state::{load_state, save_state};
use stats::CrawlStats;
use std::sync::{Arc, Mutex};

fn main() {
    let start_url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let queue = Arc::new(SegQueue::new());
    let visited = Arc::new(Mutex::new(Vec::<String>::new()));
    let stats = Arc::new(Mutex::new(CrawlStats::new()));

    // Load crawl state if available
    if let Ok(state) = load_state() {
        for (url, depth) in state.queue {
            queue.push((url, depth));
        }
        let mut visited_guard = visited.lock().unwrap();
        *visited_guard = state.visited;
    } else {
        queue.push((start_url.to_string(), 0));
    }

    start_crawl(&queue, &visited, &stats);

    let visited_pages = visited.lock().unwrap();
    println!("Visited pages: {:?}", *visited_pages);
    state::save_visited(&visited_pages).expect("Failed to save visited pages");

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
    save_state(&state).expect("Failed to save crawl state");

    // Show statistics
    let stats_guard = stats.lock().unwrap();
    println!("Crawl statistics: {:?}", *stats_guard);
}
