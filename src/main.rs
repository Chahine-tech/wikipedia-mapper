use crossbeam::queue::SegQueue;
use reqwest::Error as ReqwestError;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_DEPTH: usize = 3;
const RATE_LIMIT: u64 = 200; // milliseconds

#[derive(Serialize, Deserialize)]
struct CrawlState {
    queue: Vec<(String, usize)>, // (URL, depth)
    visited: Vec<String>,
}

#[derive(Serialize, Debug, Deserialize)]
struct CrawlStats {
    pages_visited: usize,
    links_followed: usize,
    links_ignored: usize,
    start_time: u64, // Time in milliseconds since UNIX_EPOCH
}

fn current_time_millis() -> u64 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration.as_millis() as u64
}

fn fetch_page(url: &str) -> Result<String, ReqwestError> {
    let response = reqwest::blocking::get(url)?;
    let body = response.text()?;
    Ok(body)
}

fn save_state(state: &CrawlState) -> io::Result<()> {
    let serialized = serde_json::to_string(state)?;
    let mut file = File::create("crawl_state.json")?;
    file.write_all(serialized.as_bytes())?;
    Ok(())
}

fn load_state() -> io::Result<CrawlState> {
    let file = File::open("crawl_state.json")?;
    let state: CrawlState = serde_json::from_reader(file)?;
    Ok(state)
}

fn save_visited(visited: &Vec<String>) -> io::Result<()> {
    let serialized = serde_json::to_string(visited)?;
    let mut file = File::create("visited_pages.json")?;
    file.write_all(serialized.as_bytes())?;
    Ok(())
}

fn main() {
    let start_url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let queue = Arc::new(SegQueue::new());
    let visited = Arc::new(Mutex::new(Vec::<String>::new()));
    let stats = Arc::new(Mutex::new(CrawlStats {
        pages_visited: 0,
        links_followed: 0,
        links_ignored: 0,
        start_time: current_time_millis(),
    }));

    // Load crawl state
    if let Ok(state) = load_state() {
        for (url, depth) in state.queue {
            queue.push((url, depth));
        }
        let mut visited_guard = visited.lock().unwrap();
        *visited_guard = state.visited;
    } else {
        queue.push((start_url.to_string(), 0));
    }

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let queue_clone = Arc::clone(&queue);
            let visited_clone = Arc::clone(&visited);
            let stats_clone = Arc::clone(&stats);

            thread::spawn(move || {
                let mut local_visited_count = 0;
                while local_visited_count < 10 {
                    let (current_url, depth) = match queue_clone.pop() {
                        Some((url, depth)) => (url, depth),
                        None => break,
                    };

                    if depth > MAX_DEPTH {
                        continue;
                    }

                    match fetch_page(&current_url) {
                        Ok(body) => {
                            let document = Html::parse_document(&body);
                            let link_selector = Selector::parse("a").unwrap();
                            let mut visited_guard = visited_clone.lock().unwrap();
                            let mut stats_guard = stats_clone.lock().unwrap();

                            for element in document.select(&link_selector) {
                                if let Some(href) = element.value().attr("href") {
                                    let href = href.to_string();
                                    if href.starts_with("/wiki/") && !visited_guard.contains(&href)
                                    {
                                        let full_url = format!("https://en.wikipedia.org{}", href);
                                        queue_clone.push((full_url.clone(), depth + 1));
                                        visited_guard.push(full_url.clone());
                                        stats_guard.links_followed += 1;
                                    } else {
                                        stats_guard.links_ignored += 1;
                                    }
                                }
                            }

                            stats_guard.pages_visited += 1;
                            local_visited_count += 1;
                        }
                        Err(_) => {
                            eprintln!("Failed to fetch {}", current_url);
                        }
                    }

                    thread::sleep(Duration::from_millis(RATE_LIMIT));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let visited_pages = visited.lock().unwrap();
    println!("Visited pages: {:?}", *visited_pages);
    save_visited(&visited_pages).expect("Failed to save visited pages");

    // Save crawl state
    let state = CrawlState {
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

    // Print crawl statistics
    let stats_guard = stats.lock().unwrap();
    println!("Crawl statistics: {:?}", *stats_guard);
}
