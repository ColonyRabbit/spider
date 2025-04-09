//! `cargo run --example example`
extern crate spider;

use spider::tokio;
use spider::website::Website;
use std::time::Instant;

#[tokio::main]
async fn main() {
    const target: &str = "robots.txt";
    let mut website: Website = Website::new("https://www.heygoody.com/");
    website
        .configuration
        .blacklist_url
        .insert(Default::default())
        .push("https://www.heygoody.com/robots.txt".into());
    website.configuration.respect_robots_txt = true;
    website.configuration.subdomains = false;
    website.configuration.delay = 0; // Defaults to 0 ms
    website.configuration.user_agent = Some(Box::new("SpiderBot".into())); // Defaults to spider/x.y.z, where x.y.z is the library version

    let start = Instant::now();
    website.crawl().await;
    let duration = start.elapsed();

    let links = website.get_all_links_visited().await;

    for link in links.iter() {
        println!("- {:?}", link.as_ref());
    }

    println!(
        "Time elapsed in website.crawl() is: {:?} for total pages: {:?}",
        duration,
        links.len()
    )
}
