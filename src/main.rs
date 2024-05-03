use serde::Deserialize;

static MIRROR_STATUS_URL: &str = "https://archlinux.org/mirrors/status/json";

#[derive(Debug, Default, Deserialize)]
struct MirrorList {
    urls: Vec<Mirror>,
}
#[derive(Debug, Default, Deserialize)]
struct Mirror {
    /// url
    url: String,
    protocol: Protocol,
    score: f64,
    delay: Option<f64>,
    /// detailed url
    details: String,
}

impl Mirror {
    /// Update delay based on ping time.
    fn update_delay(&mut self) {}
}

#[derive(Debug, Default, Deserialize)]
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
