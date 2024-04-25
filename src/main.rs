extern crate crossbeam;
extern crate reqwest;
extern crate scraper;

use crossbeam::queue::SegQueue;
use scraper::{Html, Selector};
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let queue = Arc::new(SegQueue::new());
    let visited = Arc::new(Mutex::new(Vec::<String>::new()));

    queue.push(url.to_string());

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let queue_clone = Arc::clone(&queue);
            let visited_clone = Arc::clone(&visited);

            thread::spawn(move || {
                let mut visited_count = 0;
                while let Some(current_url) = queue_clone.pop() {
                    if visited_count >= 10 {
                        break;
                    }
                    let body = reqwest::blocking::get(&current_url)
                        .unwrap()
                        .text()
                        .unwrap();
                    let link_selector = Selector::parse("a").unwrap();

                    let document = Html::parse_document(&body);

                    let mut visited_guard = visited_clone.lock().unwrap();

                    for element in document.select(&link_selector) {
                        if let Some(href) = element.value().attr("href") {
                            let href = href.to_string();
                            if href.starts_with("/wiki/") && !visited_guard.contains(&href) {
                                let full_url = format!("https://en.wikipedia.org{}", href);
                                queue_clone.push(full_url);
                                visited_guard.push(href);
                            }
                        }
                    }

                    visited_count += 1;
                    drop(visited_guard); // Manually drop visited_guard here
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Visited pages: {:?}", *visited.lock().unwrap());
}
