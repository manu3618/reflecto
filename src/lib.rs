use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use std::fs::File;
use std::io::Write;
use std::path::Path;

static MIRROR_STATUS_URL: &str = "https://archlinux.org/mirrors/status/json";

#[derive(Debug, Default, Deserialize)]
pub struct MirrorList {
    urls: Vec<Mirror>,
}

impl MirrorList {
    pub fn from_default_url() -> Result<Self> {
        Ok(Self::from_url(MIRROR_STATUS_URL)?)
    }

    fn from_url(url: &str) -> Result<Self> {
        let body = reqwest::blocking::get(url)?.text()?;
        // XXX
        let mut file = File::create(Path::new("/tmp/json.json"))?;
        file.write_all(&body.clone().into_bytes())?;
        // XXX

        let mlist: Self = serde_json::from_str(&body).expect(&format!("malformed JSON: {}", &body));
        Ok(mlist)
    }
}

#[derive(Debug, Default, Deserialize)]
struct Mirror {
    /// url
    url: String,
    protocol: Protocol,
    score: Option<f64>,
    delay: Option<f64>,

    #[serde(default)]
    #[serde(with = "parse_date")]
    last_sync: Option<DateTime<Utc>>,
    /// detailed url
    details: String,
}

/// home made implementation of serde deserializer for dates
mod parse_date {
    use chrono::DateTime;
    use chrono::Utc;
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if let Ok(s) = String::deserialize(deserializer) {
            if let Ok(d) = DateTime::parse_from_rfc3339(&s) {
                Ok(Some(d.into()))
            } else {
                todo!()
            }
        } else {
            Ok(None)
        }
    }
}

impl Mirror {
    /// Update delay based on ping time.
    fn update_delay(&mut self) {}
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Protocol {
    FTP,
    #[default]
    HTTPS,
    HTTP,
    RSYNC,
}

#[derive(Debug)]
enum Country {}

#[cfg(test)]
mod tests {
    use super::*;

    static MIRROR0: &str = r#"
             {
                 "url": "https://mirrors.rutgers.edu/archlinux/",
                 "protocol": "https",
                 "last_sync": null,
                 "completion_pct": 0.0,
                 "delay": null,
                 "duration_avg": null,
                 "duration_stddev": null,
                 "score": null,
                 "active": true,
                 "country": "United States",
                 "country_code": "US",
                 "isos": true,
                 "ipv4": true,
                 "ipv6": false,
                 "details": "https://archlinux.org/mirrors/rutgers.edu/910/"
             }"#;
    static MIRROR1: &str = r#"
             {
                 "url": "http://ftp.ntua.gr/pub/linux/archlinux/",
                 "protocol": "http",
                 "last_sync": "2024-05-01T14:25:08Z",
                 "completion_pct": 1.0,
                 "delay": 6354,
                 "duration_avg": 0.4358575581256008,
                 "duration_stddev": 0.6512862688716142,
                 "score": 2.852143826997215,
                 "active": true,
                 "country": "Greece",
                 "country_code": "GR",
                 "isos": true,
                 "ipv4": true,
                 "ipv6": true,
                 "details": "https://archlinux.org/mirrors/ntua.gr/333/"
             }"#;

    #[test]
    fn mirror0() {
        let _: Mirror = serde_json::from_str(MIRROR0).unwrap();
    }

    #[test]
    fn mirror1() {
        let _: Mirror = serde_json::from_str(MIRROR1).unwrap();
    }
}
