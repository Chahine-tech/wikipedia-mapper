use crossbeam::queue::SegQueue;
use scraper::{Html, Selector};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    let start_url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let queue = Arc::new(SegQueue::new());
    let visited = Arc::new(Mutex::new(Vec::<String>::new()));

    queue.push(start_url.to_string());

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let queue_clone = Arc::clone(&queue);
            let visited_clone = Arc::clone(&visited);

            thread::spawn(move || {
                let mut local_visited_count = 0;
                while local_visited_count < 10 {
                    let current_url = match queue_clone.pop() {
                        Some(url) => url,
                        None => break, // Exit the loop if the queue is empty
                    };

                    if let Ok(response) = reqwest::blocking::get(&current_url) {
                        if let Ok(body) = response.text() {
                            let document = Html::parse_document(&body);
                            let link_selector = Selector::parse("a").unwrap();

                            let mut visited_guard = visited_clone.lock().unwrap();

                            for element in document.select(&link_selector) {
                                if let Some(href) = element.value().attr("href") {
                                    let href = href.to_string();
                                    if href.starts_with("/wiki/") && !visited_guard.contains(&href)
                                    {
                                        let full_url = format!("https://en.wikipedia.org{}", href);
                                        queue_clone.push(full_url.clone());
                                        visited_guard.push(full_url);
                                    }
                                }
                            }

                            local_visited_count += 1;
                        }
                    }

                    thread::sleep(Duration::from_millis(100)); // Sleep to avoid rapid requests
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let visited_pages = visited.lock().unwrap();
    println!("Visited pages: {:?}", *visited_pages);
}
