use crate::stats::CrawlStats;
use crate::utils::fetch_page;
use crate::graph::GraphExporter;
use crossbeam::queue::SegQueue;
use scraper::{Html, Selector};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;
use std::collections::HashSet;
use anyhow::Result;
use futures::future::join_all;
use tokio::time::sleep;

const MAX_DEPTH: usize = 3;
const RATE_LIMIT: u64 = 200;
const NUM_CONCURRENT_REQUESTS: usize = 10;
const MAX_PAGES_PER_WORKER: usize = 10;

#[derive(Debug)]
struct PageLinks {
    links_followed: usize,
    links_ignored: usize,
    new_urls: Vec<String>,
}

pub struct Crawler {
    queue: Arc<SegQueue<(String, usize)>>,
    visited: Arc<RwLock<HashSet<String>>>,
    stats: Arc<RwLock<CrawlStats>>,
    graph: Arc<RwLock<GraphExporter>>,
}

impl Crawler {
    pub fn new(start_url: Option<String>) -> Self {
        let queue = Arc::new(SegQueue::new());
        let visited = Arc::new(RwLock::new(HashSet::new()));
        let stats = Arc::new(RwLock::new(CrawlStats::new()));
        let graph = Arc::new(RwLock::new(GraphExporter::new()));

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
        let mut visited_guard = self.visited.write().await;
        *visited_guard = state.visited;
        Ok(())
    }

    pub async fn get_state(&self) -> Result<crate::state::CrawlState> {
        let visited_guard = self.visited.read().await;
        
        let mut queue_vec = vec![];
        while let Some(item) = self.queue.pop() {
            queue_vec.push(item);
        }

        Ok(crate::state::CrawlState {
            queue: queue_vec,
            visited: visited_guard.clone(),
        })
    }

    pub async fn get_stats(&self) -> Result<CrawlStats> {
        let stats_guard = self.stats.read().await;
        Ok(stats_guard.clone())
    }

    pub async fn export_graph(&self, dot_path: &str, json_path: &str) -> Result<()> {
        let graph_guard = self.graph.read().await;
        graph_guard.export_dot(dot_path)?;
        graph_guard.export_json(json_path)?;
        Ok(())
    }

    fn extract_links(content: &str, _base_url: &str) -> PageLinks {
        let document = Html::parse_document(content);
        let selectors = [
            "a[href^='/wiki/']",
            "#mw-content-text a[href^='/wiki/']",
            ".mw-parser-output a[href^='/wiki/']",
            "div.mw-parser-output a[href^='/wiki/']"
        ];

        let mut new_urls = Vec::new();
        let mut links_followed = 0;
        let mut links_ignored = 0;

        for selector_str in selectors {
            if let Ok(link_selector) = Selector::parse(selector_str) {
                for element in document.select(&link_selector) {
                    if let Some(href) = element.value().attr("href") {
                        let href = href.to_string();
                        
                        if !href.contains(":") && !href.contains("#") {
                            let full_url = format!("https://en.wikipedia.org{}", href);
                            new_urls.push(full_url);
                            links_followed += 1;
                        } else {
                            links_ignored += 1;
                        }
                    }
                }
            }
        }

        PageLinks {
            links_followed,
            links_ignored,
            new_urls,
        }
    }

    async fn process_page(
        queue: &SegQueue<(String, usize)>,
        visited: &Arc<RwLock<HashSet<String>>>,
        stats: &Arc<RwLock<CrawlStats>>,
        graph: &Arc<RwLock<GraphExporter>>,
        url: String,
        depth: usize,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Ok(());
        }

        // Vérifier si l'URL a déjà été visitée (lecture seule)
        {
            let visited_guard = visited.read().await;
            if visited_guard.contains(&url) {
                return Ok(());
            }
        }

        // Fetch en dehors des sections verrouillées
        let body = match fetch_page(&url).await {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Failed to fetch page {}: {}", url, e);
                return Ok(());
            }
        };

        // Marquer comme visité et mettre à jour les stats (écriture)
        {
            let mut visited_guard = visited.write().await;
            if !visited_guard.contains(&url) {
                visited_guard.insert(url.clone());
                let mut stats_guard = stats.write().await;
                stats_guard.pages_visited += 1;
            }
        }

        // Extraire les liens dans le même thread
        let page_links = Self::extract_links(&body, &url);

        // Filtrer les URLs déjà visitées
        let mut new_links = Vec::new();
        for new_url in page_links.new_urls {
            let is_new = {
                let visited_guard = visited.read().await;
                !visited_guard.contains(&new_url)
            };
            
            if is_new {
                new_links.push(new_url);
            }
        }

        // Mettre à jour le graphe
        {
            let mut graph_guard = graph.write().await;
            for new_link in &new_links {
                graph_guard.add_edge(url.clone(), new_link.clone());
                queue.push((new_link.clone(), depth + 1));
            }
        }

        // Mettre à jour les statistiques
        {
            let mut stats_guard = stats.write().await;
            stats_guard.links_followed += page_links.links_followed;
            stats_guard.links_ignored += page_links.links_ignored;
        }
        
        Ok(())
    }

    pub async fn start_crawl(&self) -> Result<()> {
        let mut tasks = Vec::new();

        {
            let mut stats_guard = self.stats.write().await;
            *stats_guard = CrawlStats::new();
        }

        if let Some((url, depth)) = self.queue.pop() {
            Self::process_page(
                &self.queue,
                &self.visited,
                &self.stats,
                &self.graph,
                url.clone(),
                depth,
            ).await?;
        }

        for _worker_id in 0..NUM_CONCURRENT_REQUESTS {
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

                    let already_visited = {
                        let visited_guard = visited_clone.read().await;
                        visited_guard.contains(&current_url)
                    };

                    if already_visited {
                        continue;
                    }

                    let result = Self::process_page(
                        &queue_clone,
                        &visited_clone,
                        &stats_clone,
                        &graph_clone,
                        current_url.clone(),
                        depth,
                    ).await;

                    if let Err(e) = result {
                        eprintln!("Failed to process {}: {}", current_url, e);
                    } else {
                        local_visited_count += 1;
                    }

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

