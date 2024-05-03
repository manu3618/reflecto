use chrono::DateTime;
use chrono::Utc;
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
    #[serde(with = "parse_date")]
    last_sync: DateTime<Utc>,
    /// detailed url
    details: String,
}

/// home made implementation of serde deserializer for dates
mod parse_date {
    use chrono::DateTime;
    use chrono::Utc;
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        if let Ok(d) = DateTime::parse_from_rfc3339(&s) {
            Ok(d.into())
        } else {
            todo!()
        }
    }
}

impl Mirror {
    /// Update delay based on ping time.
    fn update_delay(&mut self) {}
}

#[derive(Debug, Default, Deserialize)]
enum Protocol {
    #[default]
    HTTPS,
    HTTP,
    RSYNC,
}

#[derive(Debug)]
enum Country {}

fn main() {
    println!("Hello, world!");
    println!("url: https://archlinux.org/mirrors/status/json");
}
