mod crawler;
mod state;
mod stats;
mod utils;

use crate::crawler::Crawler;
use state::load_state;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let start_url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let crawler = Crawler::new(Some(start_url.to_string()));

    // Load crawl state if available
    if let Ok(state) = load_state() {
        crawler.load_state(state).await?;
    }

    crawler.start_crawl().await?;

    // Get and save visited pages
    let visited_pages = crawler.get_visited().await?;
    println!("Visited pages: {:?}", visited_pages);
    state::save_visited(&visited_pages)?;

    // Save crawl state
    let state = crawler.get_state().await?;
    state::save_state(&state)?;

    // Show statistics
    let stats = crawler.get_stats().await?;
    println!("Crawl statistics: {:?}", stats);
    
    Ok(())
}
