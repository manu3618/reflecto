use serde::Deserialize;

static MIRROR_STATUS_URL: &str = "https://archlinux.org/mirrors/status/json";

#[derive(Debug, Default, Deserialize)]
struct MirrorList {
    urls: Vec<Mirror>,
}
#[derive(Debug, Default, Deserialize)]
struct Mirror {
    url: String,
    protocol: Protocol,
    score: f64,
    details: String,
}
#[derive(Debug, Default)]
enum Protocol {
    #[default]
    HTTPS,
    RSYNC,
}

#[derive(Debug)]
enum Country {}

fn main() {
    println!("Hello, world!");
    println!("url: https://archlinux.org/mirrors/status/json");
}
