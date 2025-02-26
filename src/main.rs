mod crawler;
mod state;
mod stats;
mod utils;
mod graph;

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

    // Save crawl state
    let state = crawler.get_state().await?;
    state::save_state(&state)?;

    // Export graph visualization
    crawler.export_graph("wikipedia_graph.dot", "wikipedia_graph.json").await?;
    println!("Graph exported to wikipedia_graph.dot and wikipedia_graph.json");
    println!("To generate a PNG visualization, run: dot -Tpng wikipedia_graph.dot -o wikipedia_graph.png");

    // Show statistics
    let stats = crawler.get_stats().await?;
    println!("Crawl statistics: {:?}", stats);
    
    Ok(())
}
