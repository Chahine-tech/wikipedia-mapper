mod crawler;
mod state;
mod stats;
mod utils;
mod graph;
mod pathfinder;

use crate::crawler::Crawler;
use crate::pathfinder::PathFinder;
use state::load_state;
use anyhow::Result;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<()> {
    let start_url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let crawler = Crawler::new(Some(start_url.to_string()));

    // Load crawl state if available
    if let Ok(state) = load_state() {
        crawler.load_state(state).await?;
    }

    crawler.start_crawl().await?;

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

    // Initialize pathfinder
    println!("\nInitializing Wikipedia path finder...");
    let pathfinder = PathFinder::load_from_json("wikipedia_graph.json")?;
    
    // Analyze graph connectivity
    pathfinder.analyze_connectivity();
    
    println!("\nCalculating average path length...");
    match pathfinder.calculate_average_path_length() {
        Ok(avg) => println!("Average distance between pages: {:.2} links", avg),
        Err(e) => println!("Could not calculate average distance: {}", e),
    }

    // Interactive path finding mode
    println!("\n=== Wikipedia Path Finder ===");
    println!("Find the shortest path between any two Wikipedia pages in our graph.");
    println!("Here are some available pages in the graph:");
    
    // Show a sample of available pages
    for page in pathfinder.get_page_sample(10) {
        println!("- {}", page);
    }
    
    println!("\nPress Ctrl+C to exit or 'l' to list more pages.");

    loop {
        print!("\nStart page URL (or 'q' to quit, 'l' to list more pages): ");
        io::stdout().flush()?;
        let mut start = String::new();
        io::stdin().read_line(&mut start)?;
        let start = start.trim();
        
        if start.eq_ignore_ascii_case("q") {
            break;
        }
        
        if start.eq_ignore_ascii_case("l") {
            println!("\nHere are 20 more pages from the graph:");
            for page in pathfinder.get_page_sample(20) {
                println!("- {}", page);
            }
            continue;
        }

        print!("End page URL: ");
        io::stdout().flush()?;
        let mut end = String::new();
        io::stdin().read_line(&mut end)?;
        let end = end.trim();

        match pathfinder.find_shortest_path(start, end) {
            Ok(path) => {
                println!("\nPath found ({} steps):", path.len() - 1);
                for (i, url) in path.iter().enumerate() {
                    // Extract page title from URL for better readability
                    let title = url.split('/').last().unwrap_or(url)
                        .replace('_', " ")
                        .replace("%20", " ");
                    println!("{}. {}", i + 1, title);
                }
            }
            Err(e) => println!("Error: {}", e),
        }
    }
    
    Ok(())
}
