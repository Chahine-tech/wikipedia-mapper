use crate::stats::CrawlStats;
use crate::utils::fetch_page;
use crate::graph::GraphExporter;
use crossbeam::queue::SegQueue;
use scraper::{Html, Selector};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use std::collections::HashSet;
use anyhow::{Result, anyhow};
use futures::future::join_all;
use tokio::time::sleep;

const MAX_DEPTH: usize = 3;
const RATE_LIMIT: u64 = 200;
const NUM_CONCURRENT_REQUESTS: usize = 10;
const MAX_PAGES_PER_WORKER: usize = 10;

pub struct Crawler {
    queue: Arc<SegQueue<(String, usize)>>,
    visited: Arc<Mutex<HashSet<String>>>,
    stats: Arc<Mutex<CrawlStats>>,
    graph: Arc<Mutex<GraphExporter>>,
}

impl Crawler {
    pub fn new(start_url: Option<String>) -> Self {
        let queue = Arc::new(SegQueue::new());
        let visited = Arc::new(Mutex::new(HashSet::new()));
        let stats = Arc::new(Mutex::new(CrawlStats::new()));
        let graph = Arc::new(Mutex::new(GraphExporter::new()));

        if let Some(url) = start_url {
            queue.push((url, 0));
        }

        Self {
            queue,
            visited,
            stats,
            graph,
        }
    }

    pub async fn load_state(&self, state: crate::state::CrawlState) -> Result<()> {
        for (url, depth) in state.queue {
            self.queue.push((url, depth));
        }
        let mut visited_guard = self.visited.lock().await;
        *visited_guard = state.visited;
        Ok(())
    }

    pub async fn get_state(&self) -> Result<crate::state::CrawlState> {
        let visited_guard = self.visited.lock().await;
        
        let mut queue_vec = vec![];
        while let Some(item) = self.queue.pop() {
            queue_vec.push(item);
        }

        Ok(crate::state::CrawlState {
            queue: queue_vec,
            visited: visited_guard.clone(),
        })
    }

    pub async fn get_visited(&self) -> Result<HashSet<String>> {
        let visited_guard = self.visited.lock().await;
        Ok(visited_guard.clone())
    }

    pub async fn get_stats(&self) -> Result<CrawlStats> {
        let stats_guard = self.stats.lock().await;
        Ok(stats_guard.clone())
    }

    pub async fn export_graph(&self, dot_path: &str, json_path: &str) -> Result<()> {
        let graph_guard = self.graph.lock().await;
        graph_guard.export_dot(dot_path)?;
        graph_guard.export_json(json_path)?;
        Ok(())
    }

    async fn process_page(
        queue: &SegQueue<(String, usize)>,
        visited: &mut HashSet<String>,
        stats: &mut CrawlStats,
        graph: &mut GraphExporter,
        url: String,
        depth: usize,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Ok(());
        }

        let body = fetch_page(&url).await?;
        let document = Html::parse_document(&body);
        let link_selector = Selector::parse("a")
            .map_err(|e| anyhow!("Failed to parse link selector: {}", e))?;

        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                let href = href.to_string();
                if href.starts_with("/wiki/") && !visited.contains(&href) {
                    let full_url = format!("https://en.wikipedia.org{}", href);
                    queue.push((full_url.clone(), depth + 1));
                    visited.insert(full_url.clone());
                    graph.add_edge(url.clone(), full_url);
                    stats.links_followed += 1;
                } else {
                    stats.links_ignored += 1;
                }
            }
        }

        stats.pages_visited += 1;
        Ok(())
    }

    pub async fn start_crawl(&self) -> Result<()> {
        let mut tasks = Vec::new();

        for _ in 0..NUM_CONCURRENT_REQUESTS {
            let queue_clone = Arc::clone(&self.queue);
            let visited_clone = Arc::clone(&self.visited);
            let stats_clone = Arc::clone(&self.stats);
            let graph_clone = Arc::clone(&self.graph);

            let task = tokio::spawn(async move {
                let mut local_visited_count = 0;
                while local_visited_count < MAX_PAGES_PER_WORKER {
                    let (current_url, depth) = match queue_clone.pop() {
                        Some((url, depth)) => (url, depth),
                        None => break,
                    };

                    let mut visited_guard = visited_clone.lock().await;
                    let mut stats_guard = stats_clone.lock().await;
                    let mut graph_guard = graph_clone.lock().await;

                    if let Err(e) = Self::process_page(
                        &queue_clone,
                        &mut visited_guard,
                        &mut stats_guard,
                        &mut graph_guard,
                        current_url.clone(),
                        depth,
                    ).await {
                        eprintln!("Failed to process {}: {}", current_url, e);
                    } else {
                        local_visited_count += 1;
                    }

                    drop(graph_guard);
                    drop(visited_guard);
                    drop(stats_guard);

                    sleep(Duration::from_millis(RATE_LIMIT)).await;
                }
                Ok::<(), anyhow::Error>(())
            });

            tasks.push(task);
        }

        for result in join_all(tasks).await {
            match result {
                Ok(Ok(())) => (),
                Ok(Err(e)) => eprintln!("Task error: {}", e),
                Err(e) => eprintln!("Task panicked: {}", e),
            }
        }

        Ok(())
    }
}
