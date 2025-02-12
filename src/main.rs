mod crawler;
mod state;
mod stats;
mod utils;

use crate::crawler::Crawler;
use state::load_state;
use anyhow::Result;

fn main() -> Result<()> {
    let start_url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let crawler = Crawler::new(Some(start_url.to_string()));

    // Load crawl state if available
    if let Ok(state) = load_state() {
        crawler.load_state(state)?;
    }

    crawler.start_crawl()?;

    // Get and save visited pages
    let visited_pages = crawler.get_visited()?;
    println!("Visited pages: {:?}", visited_pages);
    state::save_visited(&visited_pages)?;

    // Save crawl state
    let state = crawler.get_state()?;
    state::save_state(&state)?;

    // Show statistics
    let stats = crawler.get_stats()?;
    println!("Crawl statistics: {:?}", stats);
    
    Ok(())
}
