use std::collections::HashSet;
use anyhow::Result;
use std::fs::File;
use std::io::Write;

#[derive(Default)]
pub struct GraphExporter {
    nodes: HashSet<String>,
    edges: Vec<(String, String)>,
}

impl GraphExporter {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_edge(&mut self, from: String, to: String) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges.push((from, to));
    }

    pub fn export_dot(&self, path: &str) -> Result<()> {
        let mut output = String::from("digraph wikipedia {\n");
        output.push_str("  // Configuration du graphe\n");
        output.push_str("  graph [rankdir=LR];\n");  
        output.push_str("  node [shape=box, style=rounded];\n");  
        output.push_str("  edge [color=gray50];\n\n"); 

        // Write nodes
        output.push_str("  // Nœuds\n");
        for node in &self.nodes {
            let label = node.split('/').last().unwrap_or(node);
            output.push_str(&format!("  \"{}\" [label=\"{}\"];\n", node, label));
        }

        // Write edges
        output.push_str("\n  // Arêtes\n");
        for (from, to) in &self.edges {
            output.push_str(&format!("  \"{}\" -> \"{}\";\n", from, to));
        }

        output.push_str("}\n");
        
        let mut file = File::create(path)?;
        file.write_all(output.as_bytes())?;
        Ok(())
    }

    pub fn export_json(&self, path: &str) -> Result<()> {
        let graph_data = serde_json::json!({
            "nodes": self.nodes.iter().collect::<Vec<_>>(),
            "edges": self.edges,
        });

        let mut file = File::create(path)?;
        file.write_all(serde_json::to_string_pretty(&graph_data)?.as_bytes())?;
        Ok(())
    }
} 