use crate::stats::CrawlStats;
use crate::utils::fetch_page;
use crossbeam::queue::SegQueue;
use scraper::{Html, Selector};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::collections::HashSet;
use anyhow::{Result, anyhow};

const MAX_DEPTH: usize = 3;
const RATE_LIMIT: u64 = 200;
const NUM_THREADS: usize = 4;
const MAX_PAGES_PER_THREAD: usize = 10;

pub struct Crawler {
    queue: Arc<SegQueue<(String, usize)>>,
    visited: Arc<Mutex<HashSet<String>>>,
    stats: Arc<Mutex<CrawlStats>>,
}

impl Crawler {
    pub fn new(start_url: Option<String>) -> Self {
        let queue = Arc::new(SegQueue::new());
        let visited = Arc::new(Mutex::new(HashSet::new()));
        let stats = Arc::new(Mutex::new(CrawlStats::new()));

        if let Some(url) = start_url {
            queue.push((url, 0));
        }

        Self {
            queue,
            visited,
            stats,
        }
    }

    pub fn load_state(&self, state: crate::state::CrawlState) -> Result<()> {
        for (url, depth) in state.queue {
            self.queue.push((url, depth));
        }
        let mut visited_guard = self.visited.lock()
            .map_err(|e| anyhow!("Failed to acquire visited lock: {}", e))?;
        *visited_guard = state.visited;
        Ok(())
    }

    pub fn get_state(&self) -> Result<crate::state::CrawlState> {
        let visited_guard = self.visited.lock()
            .map_err(|e| anyhow!("Failed to acquire visited lock: {}", e))?;
        
        let mut queue_vec = vec![];
        while let Some(item) = self.queue.pop() {
            queue_vec.push(item);
        }

        Ok(crate::state::CrawlState {
            queue: queue_vec,
            visited: visited_guard.clone(),
        })
    }

    pub fn get_visited(&self) -> Result<HashSet<String>> {
        let visited_guard = self.visited.lock()
            .map_err(|e| anyhow!("Failed to acquire visited lock: {}", e))?;
        Ok(visited_guard.clone())
    }

    pub fn get_stats(&self) -> Result<CrawlStats> {
        let stats_guard = self.stats.lock()
            .map_err(|e| anyhow!("Failed to acquire stats lock: {}", e))?;
        Ok(stats_guard.clone())
    }

    fn process_page(
        queue: &SegQueue<(String, usize)>,
        visited: &mut HashSet<String>,
        stats: &mut CrawlStats,
        url: String,
        depth: usize,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Ok(());
        }

        let body = fetch_page(&url)?;
        let document = Html::parse_document(&body);
        let link_selector = Selector::parse("a")
            .map_err(|e| anyhow!("Failed to parse link selector: {}", e))?;

        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                let href = href.to_string();
                if href.starts_with("/wiki/") && !visited.contains(&href) {
                    let full_url = format!("https://en.wikipedia.org{}", href);
                    queue.push((full_url.clone(), depth + 1));
                    visited.insert(full_url);
                    stats.links_followed += 1;
                } else {
                    stats.links_ignored += 1;
                }
            }
        }

        stats.pages_visited += 1;
        Ok(())
    }

    pub fn start_crawl(&self) -> Result<()> {
        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|_| {
                let queue_clone = Arc::clone(&self.queue);
                let visited_clone = Arc::clone(&self.visited);
                let stats_clone = Arc::clone(&self.stats);

                thread::spawn(move || -> Result<()> {
                    let mut local_visited_count = 0;
                    while local_visited_count < MAX_PAGES_PER_THREAD {
                        let (current_url, depth) = match queue_clone.pop() {
                            Some((url, depth)) => (url, depth),
                            None => break,
                        };

                        let mut visited_guard = visited_clone.lock()
                            .map_err(|e| anyhow!("Failed to acquire visited lock: {}", e))?;
                        let mut stats_guard = stats_clone.lock()
                            .map_err(|e| anyhow!("Failed to acquire stats lock: {}", e))?;

                        if let Err(e) = Self::process_page(
                            &queue_clone,
                            &mut visited_guard,
                            &mut stats_guard,
                            current_url.clone(),
                            depth,
                        ) {
                            eprintln!("Failed to process {}: {}", current_url, e);
                        } else {
                            local_visited_count += 1;
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
}
