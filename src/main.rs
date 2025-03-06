mod crawler;
mod state;
mod stats;
mod utils;
mod graph;
mod pathfinder;
mod analytics;

use crate::crawler::Crawler;
use crate::pathfinder::PathFinder;
use crate::analytics::Analytics;
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

    // After creating the crawler
    // Add custom URL filtering rules before starting the crawl
    crawler.add_custom_url_rule("exclude_years", Box::new(|url| {
        // Exclude pages that are just years (e.g., /wiki/1999)
        !url.matches(r"/wiki/\d{4}$").next().is_some()
    })).await;

    crawler.add_custom_url_rule("exclude_dates", Box::new(|url| {
        // Exclude pages that are dates (e.g., /wiki/January_1)
        let months = ["January", "February", "March", "April", "May", "June",
                     "July", "August", "September", "October", "November", "December"];
        !months.iter().any(|month| url.contains(&format!("/wiki/{}_", month)))
    })).await;

    // Start the crawl
    crawler.start_crawl().await?;

    // Display final statistics
    let stats = crawler.get_stats().await?;
    println!("\nCrawl completed!");
    println!("Pages visited: {}", stats.pages_visited);
    println!("Links followed: {}", stats.links_followed);
    println!("Links ignored: {}", stats.links_ignored);

    // Save state for potential resume
    let state = crawler.get_state().await?;
    state::save_state(&state)?;

    // Export graph visualization
    crawler.export_graph("wikipedia_graph.dot", "wikipedia_graph.json").await?;
    println!("Graph exported to wikipedia_graph.dot and wikipedia_graph.json");
    println!("To generate a PNG visualization, run: dot -Tpng wikipedia_graph.dot -o wikipedia_graph.png");

    // Show statistics
    let stats = crawler.get_stats().await?;
    println!("Crawl statistics: {:?}", stats);

    // After exporting graph and before pathfinder initialization, add:
    println!("\nCalculating PageRank for all pages...");
    let graph_data = std::fs::read_to_string("wikipedia_graph.json")?;
    let graph: serde_json::Value = serde_json::from_str(&graph_data)?;
    
    let mut analytics = Analytics::new();
    if let Some(edges) = graph["edges"].as_array() {
        let edges: Vec<(String, String)> = edges
            .iter()
            .filter_map(|edge| {
                if let Some(edge_array) = edge.as_array() {
                    if edge_array.len() >= 2 {
                        Some((
                            edge_array[0].as_str()?.to_string(),
                            edge_array[1].as_str()?.to_string(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        
        println!("Loaded {} edges for PageRank calculation", edges.len());
        analytics.load_from_edges(edges);
        
        match analytics.calculate_pagerank() {
            Ok(results) => {
                println!("PageRank calculation completed in {} iterations (converged: {})", 
                    results.iterations, results.converged);
                
                // Sort pages by PageRank score
                let mut pages: Vec<_> = results.scores.into_iter().collect();
                pages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                
                println!("\nTop 10 most important pages:");
                for (i, (page, score)) in pages.iter().take(10).enumerate() {
                    let title = page.split('/').last().unwrap_or(page)
                        .replace('_', " ")
                        .replace("%20", " ");
                    println!("{}. {} (score: {:.6})", i + 1, title, score);
                }
            }
            Err(e) => println!("Error calculating PageRank: {}", e),
        }
    }

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
