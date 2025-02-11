use crate::stats::CrawlStats;
use crate::utils::fetch_page;
use crossbeam::queue::SegQueue;
use scraper::{Html, Selector};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use anyhow::{Result, anyhow};

const MAX_DEPTH: usize = 3;
const RATE_LIMIT: u64 = 200;

pub fn start_crawl(
    queue: &Arc<SegQueue<(String, usize)>>,
    visited: &Arc<Mutex<Vec<String>>>,
    stats: &Arc<Mutex<CrawlStats>>,
) -> Result<()> {
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let queue_clone = Arc::clone(queue);
            let visited_clone = Arc::clone(visited);
            let stats_clone = Arc::clone(stats);

            thread::spawn(move || -> Result<()> {
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
                            let link_selector = Selector::parse("a")
                                .map_err(|e| anyhow!("Failed to parse link selector: {}", e))?;
                            let mut visited_guard = visited_clone.lock()
                                .map_err(|e| anyhow!("Failed to acquire visited lock: {}", e))?;
                            let mut stats_guard = stats_clone.lock()
                                .map_err(|e| anyhow!("Failed to acquire stats lock: {}", e))?;

                            for element in document.select(&link_selector) {
                                if let Some(href) = element.value().attr("href") {
                                    let href = href.to_string();
                                    if href.starts_with("/wiki/") && !visited_guard.contains(&href) {
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
                        Err(e) => {
                            eprintln!("Failed to fetch {}: {}", current_url, e);
                        }
                    }

                    thread::sleep(Duration::from_millis(RATE_LIMIT));
                }
                Ok(())
            })
        })
        .collect();

    for handle in handles {
        handle.join()
            .map_err(|e| anyhow!("Thread panicked: {:?}", e))??;
    }

    Ok(())
}
