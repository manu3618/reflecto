use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use std::fs::File;
use std::io::Write;
use std::path::Path;

static MIRROR_STATUS_URL: &str = "https://archlinux.org/mirrors/status/json";
#[derive(Debug)]
enum SortKey {
    /// Last server syncrhonisation
    Age,
    /// Download reate
    Rate,
    /// Country name, alphabetically
    Country,
    /// Mirror status score
    Score,
    /// Mirror status delay
    Delay,
}
#[derive(Debug, Default, Deserialize)]
pub struct MirrorList {
    #[serde(rename = "urls")]
    mirrors: Vec<Mirror>,

    #[serde(default)]
    source: Option<String>,
}

impl MirrorList {
    pub fn from_default_url() -> Result<Self> {
        Self::from_url(MIRROR_STATUS_URL)
    }

    fn from_url(url: &str) -> Result<Self> {
        let body = reqwest::blocking::get(url)?.text()?;
        // XXX
        let mut file = File::create(Path::new("/tmp/json.json"))?;
        file.write_all(&body.clone().into_bytes())?;
        // XXX

        let mut mlist: Self =
            serde_json::from_str(&body).unwrap_or_else(|_| panic!("malformed JSON: {}", &body));
        mlist.source = Some(url.into());
        Ok(mlist)
    }

    /// Sort mirrors by sortkey
    fn sort(&mut self, by: SortKey) {
        match by {
            SortKey::Age => todo!(),
            SortKey::Rate => todo!(),
            SortKey::Country => todo!(),
            SortKey::Score => todo!(),
            SortKey::Delay => self
                .mirrors
                .sort_by_key(|m| m.delay.unwrap_or(f64::INFINITY).round() as i32),
        }
    }

    /// return the content to put in mirrorlist
    pub fn to_file_content(&self) -> String {
        let mut lines = vec![self.file_preambule(), "".into()];
        lines.push(self.server_list(usize::MAX)); // TODO: add limit
        lines.join("\n")
    }

    ///generate the file preambule
    fn file_preambule(&self) -> String {
        let mut lines: Vec<String> = vec![
            "# Arch Linux mirror list generated by reflecto.rs".into(),
            "#".into(),
        ];
        // TODO: add status lines (date, program name,...)
        if let Some(s) = &self.source {
            lines.push(format!("# from: \t{s}"));
        }
        lines.join("\n")
    }

    fn server_list(&self, limit: usize) -> String {
        let limit = if limit > self.mirrors.len() {
            self.mirrors.len()
        } else {
            limit
        };

        self.mirrors[0..limit]
            .iter()
            .map(|m| format!("Server = {}$repo/os/$arch", m.url))
            .collect::<Vec<_>>()
            .join("\n")
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
    Ftp,
    #[default]
    Https,
    Http,
    Rsync,
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
    static MIRROR2: &str = r#"
            {
                "url": "https://mirror.aarnet.edu.au/pub/archlinux/",
                "protocol": "https",
                "last_sync": "2024-04-01T08:22:54Z",
                "completion_pct": 1.0,
                "delay": 1863,
                "duration_avg": 1.1129106909958357,
                "duration_stddev": 0.23354254068513589,
                "score": 1.8639532316809715,
                "active": true,
                "country": "Australia",
                "country_code": "AU",
                "isos": true,
                "ipv4": true,
                "ipv6": true,
                "details": "https://archlinux.org/mirrors/aarnet.edu.au/5/"
            }"#;

    #[test]
    fn mirror0() {
        let _: Mirror = serde_json::from_str(MIRROR0).unwrap();
    }

    #[test]
    fn mirror1() {
        let _: Mirror = serde_json::from_str(MIRROR1).unwrap();
    }

    #[test]
    fn sort_delay() {
        let j = format!("{{\"urls\":[{MIRROR0},{MIRROR1},{MIRROR2}]}}");
        let mut ml: MirrorList = serde_json::from_str(&j).unwrap();
        ml.sort(SortKey::Delay);
        assert_eq!(
            ml.mirrors[0].url,
            "https://mirror.aarnet.edu.au/pub/archlinux/"
        );
        assert_eq!(ml.mirrors[2].url, "https://mirrors.rutgers.edu/archlinux/");
    }
}
