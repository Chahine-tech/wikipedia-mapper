use anyhow::Error;
use dot::Graph;
use petgraph::Graph as PetGraph;
use reqwest::blocking::get;
use scraper::{Html, Selector};
use std::collections::VecDeque;
use std::sync::mpsc::channel;
use std::thread;

fn main() -> Result<(), Error> {
    let url = "https://fr.wikipedia.org/wiki/Rust_(langage)";

    let mut queue = VecDeque::new();
    queue.push_back(url.to_string());

    let (tx, rx) = channel();

    let mut graph = PetGraph::<String, ()>::new();

    while let Some(page_url) = queue.pop_front() {
        let tx = tx.clone();

        thread::spawn(move || -> Result<(), Error> {
            let html = get(page_url.clone())?.text()?;

            let document = Html::parse_document(&html);
            let selector = Selector::parse("a").unwrap();
            for element in document.select(&selector) {
                if let Some(link) = element.value().attr("href") {
                    let link = link.trim();
                    if link.starts_with("/wiki/") && !link.contains(":") {
                        let page_url = format!("https://fr.wikipedia.org{}", link);
                        queue.push_back(page_url.clone());
                        tx.send(page_url)?;
                    }
                }
            }

            let page_title = document
                .select(&Selector::parse("h1#firstHeading").unwrap())
                .next()
                .unwrap()
                .text()
                .collect::<Vec<_>>()
                .join("");
            let node_index = graph.add_node(page_title);

            for neighbor in graph.neighbors(node_index) {
                graph.add_edge(node_index, neighbor, ());
            }

            Ok(())
        });
    }

    for _ in 0..queue.len() {
        rx.recv()?;
    }

    let dot = {
        let mut g = Graph::new("G");
        g.set_node_labels(true);
        g.set_edge_labels(true);

        for node in graph.raw_nodes().iter() {
            g.add_node(node.weight);
        }

        for edge in graph.raw_edges().iter() {
            g.add_edge(edge.source().index(), edge.target().index(), "");
        }

        Dot::new_digraph(&g)
    };

    println!("{}", dot);

    Ok(())
}
