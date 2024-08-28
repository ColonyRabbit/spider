//! `cargo run --example chrome --features chrome`
extern crate spider;

use std::time::Duration;

use spider::configuration::WaitForIdleNetwork;
use spider::tokio;
use spider::website::Website;

#[tokio::main]
async fn main() {
    let mut website: Website = Website::new("https://choosealicense.com")
        .with_chrome_intercept(true, true)
        .with_limit(5)
        .with_wait_for_idle_network(Some(WaitForIdleNetwork::new(Some(Duration::from_secs(30)))))
        .with_caching(cfg!(feature = "cache"))
        // // you can use the project [https://github.com/spider-rs/chrome-server] to spin a chrome server locally to connect to.
        // .with_chrome_connection(Some("http://127.0.0.1:6000/json/version".into()))
        .build()
        .unwrap();
    let mut rx2 = website.subscribe(16).unwrap();

    tokio::spawn(async move {
        while let Ok(page) = rx2.recv().await {
            println!("{:?}", page.get_url());
        }
    });

    let start = crate::tokio::time::Instant::now();
    website.crawl().await;

    let duration = start.elapsed();

    let links = website.get_links();

    println!(
        "Time elapsed in website.crawl() is: {:?} for total pages: {:?}",
        duration,
        links.len()
    )
}
