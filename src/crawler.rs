use crate::stats::CrawlStats;
use crate::utils::fetch_page;
use crate::graph::GraphExporter;
use crossbeam::queue::SegQueue;
use scraper::{Html, Selector};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;
use std::collections::{HashSet, HashMap};
use anyhow::Result;
use futures::future::join_all;
use tokio::time::sleep;
use regex::Regex;
use lazy_static::lazy_static;

const MAX_DEPTH: usize = 3;
const RATE_LIMIT: u64 = 200;
const NUM_CONCURRENT_REQUESTS: usize = 10;
const MAX_PAGES_PER_WORKER: usize = 10;

lazy_static! {
    static ref INVALID_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"Wikipedia:(.*?)").unwrap(),
        Regex::new(r"Special:(.*?)").unwrap(),
        Regex::new(r"Talk:(.*?)").unwrap(),
        Regex::new(r"User:(.*?)").unwrap(),
        Regex::new(r"File:(.*?)").unwrap(),
        Regex::new(r"Template:(.*?)").unwrap(),
        Regex::new(r"Help:(.*?)").unwrap(),
        Regex::new(r"Category:(.*?)").unwrap(),
        Regex::new(r"Portal:(.*?)").unwrap(),
    ];
}

struct DebugFn(Box<dyn Fn(&str) -> bool + Send + Sync>);

impl DebugFn {
    fn new(f: Box<dyn Fn(&str) -> bool + Send + Sync>) -> Self {
        Self(f)
    }

    fn call(&self, input: &str) -> bool {
        (self.0)(input)
    }
}

impl std::fmt::Debug for URLFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("URLFilter")
            .field("allowed_domains", &self.allowed_domains)
            .field("excluded_patterns", &self.excluded_patterns)
            .field("path_prefixes", &self.path_prefixes)
            .field("custom_rules_count", &self.custom_rules.len())
            .finish()
    }
}

struct URLFilter {
    allowed_domains: HashSet<String>,
    excluded_patterns: Vec<Regex>,
    path_prefixes: HashSet<String>,
    custom_rules: HashMap<String, DebugFn>,
}

impl Default for URLFilter {
    fn default() -> Self {
        let mut allowed_domains = HashSet::new();
        allowed_domains.insert("en.wikipedia.org".to_string());

        let mut path_prefixes = HashSet::new();
        path_prefixes.insert("/wiki/".to_string());

        Self {
            allowed_domains,
            excluded_patterns: INVALID_PATTERNS.to_vec(),
            path_prefixes,
            custom_rules: HashMap::new(),
        }
    }
}

impl URLFilter {
    fn is_valid_url(&self, url: &str) -> bool {
        // Check if URL contains a fragment
        if url.contains('#') {
            return false;
        }

        // Check domain
        let url_parsed = match url::Url::parse(url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        // Check if domain is allowed
        if !self.allowed_domains.contains(url_parsed.host_str().unwrap_or("")) {
            return false;
        }

        // Check path prefixes
        let path = url_parsed.path();
        if !self.path_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
            return false;
        }

        // Check excluded patterns
        for pattern in &self.excluded_patterns {
            if pattern.is_match(path) {
                return false;
            }
        }

        // Apply custom rules
        for rule in self.custom_rules.values() {
            if !rule.call(url) {
                return false;
            }
        }

        true
    }

    fn add_custom_rule(&mut self, name: &str, rule: Box<dyn Fn(&str) -> bool + Send + Sync>) {
        self.custom_rules.insert(name.to_string(), DebugFn::new(rule));
    }
}

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
    url_filter: Arc<RwLock<URLFilter>>,
}

impl Crawler {
    pub fn new(start_url: Option<String>) -> Self {
        let queue = Arc::new(SegQueue::new());
        let visited = Arc::new(RwLock::new(HashSet::new()));
        let stats = Arc::new(RwLock::new(CrawlStats::new()));
        let graph = Arc::new(RwLock::new(GraphExporter::new()));
        let url_filter = Arc::new(RwLock::new(URLFilter::default()));

        if let Some(url) = start_url {
            queue.push((url, 0));
        }

        Self {
            queue,
            visited,
            stats,
            graph,
            url_filter,
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

    fn extract_links(content: &str, url_filter: &URLFilter) -> PageLinks {
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
                        let full_url = format!("https://en.wikipedia.org{}", href);
                        
                        if url_filter.is_valid_url(&full_url) {
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
        url_filter: &Arc<RwLock<URLFilter>>,
        url: String,
        depth: usize,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Ok(());
        }

        // Check if URL is valid
        {
            let filter = url_filter.read().await;
            if !filter.is_valid_url(&url) {
                return Ok(());
            }
        }

        // Check if URL has already been visited (read-only)
        {
            let visited_guard = visited.read().await;
            if visited_guard.contains(&url) {
                return Ok(());
            }
        }

        // Fetch outside of locked sections
        let body = match fetch_page(&url).await {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Failed to fetch page {}: {}", url, e);
                return Ok(());
            }
        };

        // Mark as visited and update stats (write)
        {
            let mut visited_guard = visited.write().await;
            if !visited_guard.contains(&url) {
                visited_guard.insert(url.clone());
                let mut stats_guard = stats.write().await;
                stats_guard.pages_visited += 1;
            }
        }

        // Extract links in the same thread
        let filter = url_filter.read().await;
        let page_links = Self::extract_links(&body, &filter);

        // Filter already visited URLs
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

        // Update the graph
        {
            let mut graph_guard = graph.write().await;
            for new_link in &new_links {
                graph_guard.add_edge(url.clone(), new_link.clone());
                queue.push((new_link.clone(), depth + 1));
            }
        }

        // Update statistics
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
                &self.url_filter,
                url.clone(),
                depth,
            ).await?;
        }

        for _worker_id in 0..NUM_CONCURRENT_REQUESTS {
            let queue_clone = Arc::clone(&self.queue);
            let visited_clone = Arc::clone(&self.visited);
            let stats_clone = Arc::clone(&self.stats);
            let graph_clone = Arc::clone(&self.graph);
            let url_filter_clone = Arc::clone(&self.url_filter);

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
                        &url_filter_clone,
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

    pub async fn add_custom_url_rule(&self, name: &str, rule: Box<dyn Fn(&str) -> bool + Send + Sync>) {
        let mut filter = self.url_filter.write().await;
        filter.add_custom_rule(name, rule);
    }
}

