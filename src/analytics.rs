use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const DAMPING_FACTOR: f64 = 0.85;
const MAX_ITERATIONS: usize = 100;
const CONVERGENCE_THRESHOLD: f64 = 1e-8;

#[derive(Debug, Serialize, Deserialize)]
pub struct PageRankResults {
    pub scores: HashMap<String, f64>,
    pub iterations: usize,
    pub converged: bool,
}

pub struct Analytics {
    // Adjacency list representation: page -> list of pages it links to
    outbound_links: HashMap<String, Vec<String>>,
    // Reverse adjacency list: page -> list of pages that link to it
    inbound_links: HashMap<String, Vec<String>>,
}

impl Analytics {
    pub fn new() -> Self {
        Self {
            outbound_links: HashMap::new(),
            inbound_links: HashMap::new(),
        }
    }

    pub fn load_from_edges(&mut self, edges: Vec<(String, String)>) {
        for (from, to) in edges {
            self.outbound_links
                .entry(from.clone())
                .or_insert_with(Vec::new)
                .push(to.clone());

            self.inbound_links
                .entry(to)
                .or_insert_with(Vec::new)
                .push(from);
        }
    }

    pub fn calculate_pagerank(&self) -> Result<PageRankResults> {
        let num_pages = self.get_all_pages().len();
        let initial_score = 1.0 / num_pages as f64;

        // Initialize scores
        let mut scores: HashMap<String, f64> = self
            .get_all_pages()
            .into_iter()
            .map(|page| (page, initial_score))
            .collect();

        let mut iterations = 0;
        let mut converged = false;

        while iterations < MAX_ITERATIONS {
            let mut new_scores: HashMap<String, f64> = HashMap::new();
            let mut total_diff = 0.0;

            // Calculate new score for each page
            for page in self.get_all_pages() {
                let mut new_score = (1.0 - DAMPING_FACTOR) / num_pages as f64;

                // Sum contributions from pages that link to this page
                if let Some(incoming) = self.inbound_links.get(&page) {
                    for source in incoming {
                        let source_score = scores.get(source).unwrap_or(&initial_score);
                        let source_outbound_count = self
                            .outbound_links
                            .get(source)
                            .map(|links| links.len())
                            .unwrap_or(1);

                        new_score += DAMPING_FACTOR * source_score / source_outbound_count as f64;
                    }
                }

                let old_score = scores.get(&page).unwrap_or(&initial_score);
                total_diff += (new_score - old_score).abs();
                new_scores.insert(page, new_score);
            }

            // Check for convergence
            if total_diff < CONVERGENCE_THRESHOLD {
                converged = true;
                break;
            }

            scores = new_scores;
            iterations += 1;
        }

        Ok(PageRankResults {
            scores,
            iterations,
            converged,
        })
    }

    fn get_all_pages(&self) -> Vec<String> {
        let mut pages: Vec<String> = self
            .outbound_links
            .keys()
            .chain(self.inbound_links.keys())
            .cloned()
            .collect();
        pages.sort();
        pages.dedup();
        pages
    }
}
