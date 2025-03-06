use anyhow::{anyhow, Result};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct PathFinder {
    // Stores links between pages (source page -> destination pages)
    adjacency_list: HashMap<String, Vec<String>>,
}

impl PathFinder {
    pub fn new() -> Self {
        Self {
            adjacency_list: HashMap::new(),
        }
    }

    // Lists all available pages in the graph
    pub fn list_pages(&self) -> Vec<&String> {
        let mut pages: Vec<&String> = self.adjacency_list.keys().collect();
        pages.sort(); // Sort alphabetically for easier reading
        pages
    }

    // Gets a sample of available pages
    pub fn get_page_sample(&self, count: usize) -> Vec<&String> {
        let mut pages = self.list_pages();
        let mut rng = thread_rng();
        pages.shuffle(&mut rng);
        pages.truncate(count.min(pages.len()));
        pages.sort(); // Sort after truncating for easier reading
        pages
    }

    // Adds a bidirectional link between two pages
    pub fn add_edge(&mut self, from: String, to: String) {
        // Add from -> to edge
        self.adjacency_list
            .entry(from.clone())
            .or_insert_with(Vec::new)
            .push(to.clone());

        // Add to -> from edge (make it bidirectional)
        self.adjacency_list
            .entry(to)
            .or_insert_with(Vec::new)
            .push(from);
    }

    // Reconstructs the path from BFS result
    fn reconstruct_path(
        came_from: &HashMap<String, String>,
        start: &str,
        end: &str,
    ) -> Option<Vec<String>> {
        if !came_from.contains_key(end) {
            return None;
        }

        let mut path = vec![end.to_string()];
        let mut current = end;

        while current != start {
            current = came_from.get(current)?;
            path.push(current.to_string());
        }

        path.reverse();
        Some(path)
    }

    // Finds the shortest path between two pages
    pub fn find_shortest_path(&self, start: &str, end: &str) -> Result<Vec<String>> {
        if !self.adjacency_list.contains_key(start) {
            return Err(anyhow!("Start page '{}' not found in graph", start));
        }
        if !self.adjacency_list.contains_key(end) {
            return Err(anyhow!("End page '{}' not found in graph", end));
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut came_from = HashMap::new();

        queue.push_back(start.to_string());
        visited.insert(start.to_string());

        while let Some(current) = queue.pop_front() {
            if current == end {
                return Self::reconstruct_path(&came_from, start, end)
                    .ok_or_else(|| anyhow!("Failed to reconstruct path"));
            }

            if let Some(neighbors) = self.adjacency_list.get(&current) {
                for next in neighbors {
                    if !visited.contains(next) {
                        visited.insert(next.clone());
                        came_from.insert(next.clone(), current.clone());
                        queue.push_back(next.clone());
                    }
                }
            }
        }

        Err(anyhow!("No path found between '{}' and '{}'", start, end))
    }

    // Calculates the average path length between a random sample of nodes
    pub fn calculate_average_path_length(&self) -> Result<f64> {
        let nodes: Vec<&String> = self.adjacency_list.keys().collect();
        let n = nodes.len();

        if n <= 1 {
            return Err(anyhow!(
                "Graph has too few nodes to calculate average distance"
            ));
        }

        // Take a random sample of node pairs to calculate average path length
        let sample_size = 100.min(n * (n - 1) / 2); // Maximum 100 samples
        let mut rng = thread_rng();
        let mut total_distance = 0;
        let mut paths_found = 0;
        let mut attempts = 0;

        println!(
            "Calculating average path length (sampling {} pairs)...",
            sample_size
        );

        while paths_found < 10 && attempts < sample_size {
            // Try to find at least 10 valid paths
            let mut indices: Vec<usize> = (0..n).collect();
            indices.shuffle(&mut rng);
            let (i, j) = (indices[0], indices[1]);

            attempts += 1;

            if let Ok(path) = self.find_shortest_path(nodes[i], nodes[j]) {
                total_distance += path.len() - 1;
                paths_found += 1;
            }
        }

        if paths_found == 0 {
            return Err(anyhow!(
                "No paths found between sampled nodes after {} attempts",
                attempts
            ));
        }

        println!(
            "Found {} valid paths out of {} attempts",
            paths_found, attempts
        );
        Ok(total_distance as f64 / paths_found as f64)
    }

    // Loads the graph from exported data
    pub fn load_from_json(path: &str) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let data: serde_json::Value = serde_json::from_reader(file)?;

        let mut pathfinder = Self::new();

        if let Some(edges) = data["edges"].as_array() {
            for edge in edges {
                if let (Some(from), Some(to)) = (edge[0].as_str(), edge[1].as_str()) {
                    pathfinder.add_edge(from.to_string(), to.to_string());
                }
            }
        }

        Ok(pathfinder)
    }

    // Analyzes the connectivity of the graph
    pub fn analyze_connectivity(&self) {
        let total_nodes = self.adjacency_list.len();
        let mut total_edges = 0;
        let mut nodes_with_no_edges = 0;
        let mut max_edges = 0;
        let mut min_edges = usize::MAX;

        for (_node, edges) in &self.adjacency_list {
            let edge_count = edges.len();
            total_edges += edge_count;

            if edge_count == 0 {
                nodes_with_no_edges += 1;
            }

            max_edges = max_edges.max(edge_count);
            min_edges = min_edges.min(edge_count);
        }

        println!("\nGraph Analysis:");
        println!("Total nodes: {}", total_nodes);
        println!("Total edges: {}", total_edges);
        println!(
            "Average edges per node: {:.2}",
            total_edges as f64 / total_nodes as f64
        );
        println!("Nodes with no edges: {}", nodes_with_no_edges);
        println!("Max edges for a node: {}", max_edges);
        println!("Min edges for a node: {}", min_edges);
    }
}
