use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use clap::ValueEnum;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tokio::task::JoinSet;
use tracing::{debug, info, instrument, span, Level};

pub static MIRROR_STATUS_URL: &str = "https://archlinux.org/mirrors/status/json";

#[derive(Debug, Clone, ValueEnum)]
pub enum SortKey {
    /// Last server syncrhonisation
    Age,
    /// Download rate
    Rate,
    /// Country name, alphabetically
    Country,
    /// Mirror status score. The lower, the better
    Score,
    /// Mirror status delay
    Delay,
}

impl fmt::Display for SortKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SortKey::Age => write!(f, "age"),
            SortKey::Rate => write!(f, "rate"),
            SortKey::Country => write!(f, "country"),
            SortKey::Score => write!(f, "score"),
            SortKey::Delay => write!(f, "delay"),
        }
    }
}

/// Download rate
#[derive(Debug, Default, PartialEq, PartialOrd, Clone)]
struct Bandwidth(f64);

impl Bandwidth {
    fn from_duration(duration: chrono::Duration, bytes_quantity: usize) -> Self {
        if bytes_quantity == 0 {
            Self(f64::NAN)
        } else {
            Self(bytes_quantity as f64 / (1000.0 * duration.num_milliseconds() as f64))
        }
    }
}

/// List of archlinux mirror status as described in
/// <https://archlinux.org/mirrors/status/>
#[derive(Debug, Default, Clone, Deserialize)]
pub struct MirrorList {
    #[serde(rename = "urls")]
    mirrors: Vec<Mirror>,

    #[serde(default)]
    source: Option<String>,
}

impl MirrorList {
    pub async fn from_default_url() -> Result<Self> {
        Self::from_url(MIRROR_STATUS_URL).await
    }

    pub async fn from_url(url: &str) -> Result<Self> {
        let body = reqwest::get(url).await?.text().await?;

        // XXX
        let mut file = File::create(Path::new("/tmp/json.json"))?;
        file.write_all(&body.clone().into_bytes())?;
        // XXX

        let mut mlist: Self = match serde_json::from_str(&body) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("malformed JSON: {}", &body);
                return Err(e.into());
            }
        };
        mlist.source = Some(url.into());
        Ok(mlist)
    }

    /// Sort mirrors by sortkey
    pub fn sort(&mut self, by: SortKey) {
        match by {
            SortKey::Age => self
                .mirrors
                .sort_by_key(|m| m.last_sync.unwrap_or_default()),
            SortKey::Rate => self.mirrors.sort_by(|m, n| {
                // inverse m and n to sordt in desc ordoer
                n.download_rate
                    .clone()
                    .unwrap_or_default()
                    .partial_cmp(&m.download_rate.clone().unwrap_or_default())
                    .unwrap_or(Ordering::Equal)
            }),
            SortKey::Country => self
                .mirrors
                .sort_by_key(|m| m.country.clone().unwrap_or_default()),
            SortKey::Score => self
                .mirrors
                .sort_by_key(|m| m.score.unwrap_or(f64::INFINITY).round() as i32),
            SortKey::Delay => self
                .mirrors
                .sort_by_key(|m| m.delay.unwrap_or(f64::INFINITY).round() as i32),
        }
    }

    /// return the content to put in mirrorlist
    pub fn to_file_content(&self, number: usize) -> String {
        let mut lines = vec![self.file_preambule(), "".into()];
        lines.push(self.server_list(number));
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

    /// get a list of all countries in which a mirror is present
    /// returns a Hashmap<(Country, Code), Count>
    fn get_countries(&self) -> HashMap<(String, String), usize> {
        let mut countries = HashMap::new();
        for mirror in self.mirrors.iter() {
            if mirror.country.is_none() || mirror.country_code.is_none() {
                continue;
            }
            let key = (
                mirror.country.clone().unwrap(),
                mirror.country_code.clone().unwrap(),
            );
            *countries.entry(key).or_insert(0) += 1;
        }
        countries
    }

    /// get a csv-like string listing countries
    pub fn print_countries(&self) -> String {
        let mut lines = Vec::new();
        let mut countries = self.get_countries().into_iter().collect::<Vec<_>>();
        countries.sort();
        let longuest = countries
            .iter()
            .map(|c| c.0 .0.chars().count())
            .max()
            .expect("at least one country");
        let longuest = longuest.max(7); // minimal value: length of "Country"
        lines.push(format!("Country{} Code Count", " ".repeat(longuest - 7)));
        lines.push(format!("{} ---- ----", "-".repeat(longuest)));
        for c in countries {
            if c.0 .0.is_empty() {
                continue;
            }
            lines.push(get_country_line(&c.0 .0, &c.0 .1, c.1, longuest));
        }
        lines.join("\n")
    }

    #[instrument]
    pub async fn update_download_rate(&mut self, timeout: Option<chrono::Duration>, limit: usize) {
        let mut left = self.mirrors.len().min(limit);
        let mut mirrors = Vec::new();
        let mut set = JoinSet::new();
        for m in self.mirrors.drain(..) {
            mirrors.push(m.clone());
            set.spawn(m.update_download_rate(timeout));
        }
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(m)) => {
                    let _ = &self.mirrors.push(m);
                    left -= 1;
                }
                _ => {
                    debug!("failed to update a mirror")
                }
            }
            if left == 0 {
                debug!("enough mirror updated");
                break;
            }
        }
        set.shutdown().await;

        // push not updated mirrors
        let ok_urls = &self
            .mirrors
            .iter()
            .map(|m| m.url.clone())
            .collect::<Vec<_>>();
        self.mirrors.append(
            &mut mirrors
                .into_iter()
                .filter(|m| !ok_urls.contains(&m.url))
                .collect::<Vec<_>>(),
        );
    }

    /// Filter out mirrors based on criteria:
    /// age: filter out mirrors not synchronized in the last n hours
    /// isos: if true, return only ISOs hosts
    /// ipv4: if true, return only ipv4 hosts
    /// ipv6: if true, return only ipv6 hosts
    /// protocol: if any, retun only those protocols
    pub fn filter(
        self,
        age: Option<f64>,
        isos: bool,
        ipv4: bool,
        ipv6: bool,
        protocol: &[Protocol],
    ) -> Self {
        let mut ml = self.mirrors;
        if let Some(age) = age {
            ml.retain(|m| match m.age() {
                Some(d) => d.num_hours() as f64 + d.num_minutes() as f64 / 60.0 < age,
                _ => false,
            });
        }
        if isos {
            ml.retain(|m| m.isos.unwrap_or(false))
        }
        if ipv4 {
            ml.retain(|m| m.ipv4.unwrap_or(false))
        }
        if ipv6 {
            ml.retain(|m| m.ipv6.unwrap_or(false))
        }
        if !protocol.is_empty() {
            ml.retain(|m| protocol.contains(&m.protocol))
        }

        Self {
            mirrors: ml,
            ..self
        }
    }
}

fn get_country_line(country: &str, code: &str, count: usize, country_len: usize) -> String {
    debug_assert!(country_len >= country.chars().count());
    let padding = " ".repeat(country_len - country.chars().count());
    debug_assert!(code.len() == 2);
    format!("{}{} {: >4} {: >4}", country, padding, code, count)
}

#[derive(Debug, Default, Clone, Deserialize)]
struct Mirror {
    /// url
    url: String,
    protocol: Protocol,
    score: Option<f64>,
    delay: Option<f64>,
    country: Option<String>,
    country_code: Option<String>,

    #[serde(default)]
    #[serde(with = "parse_date")]
    last_sync: Option<DateTime<Utc>>,

    isos: Option<bool>,
    ipv4: Option<bool>,
    ipv6: Option<bool>,
    /// detailed url
    details: String,

    #[serde(skip)]
    download_rate: Option<Bandwidth>,
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
    /// Update download rate.
    async fn update_dl_rate(&mut self, timeout: Option<chrono::Duration>) -> Result<()> {
        let span = span!(Level::DEBUG, "update download rate", url = self.url.clone());
        let _guard = span.enter();
        let client = match timeout {
            Some(d) => reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(
                    d.num_seconds().try_into().unwrap(),
                ))
                .build()?,
            None => reqwest::Client::new(),
        };
        let now = Utc::now();
        let response = client
            .get(format!("{}/extra/os/x86_64/extra.db", self.url))
            .send()
            .await?;
        let content = match response.bytes().await {
            Ok(c) => c,
            Err(e) => {
                // TODO: get the first bytes received before the timeout
                debug!("{:?}", &e);
                return Err(e.into());
            }
        };
        let end = Utc::now();
        self.download_rate = Some(Bandwidth::from_duration(end - now, content.len()));
        info!("donwload rate updated for url {}", self.url.clone());
        Ok(())
    }

    /// Update download rate. Function that can be used by MirrorList
    async fn update_download_rate(mut self, timeout: Option<chrono::Duration>) -> Result<Self> {
        self.update_dl_rate(timeout).await?;
        Ok(self)
    }

    /// Compute mirror age based on last server synchronisation
    fn age(&self) -> Option<chrono::Duration> {
        self.last_sync.map(|last_sync| Utc::now() - last_sync)
    }
}

#[derive(Debug, Default, Clone, Deserialize, ValueEnum, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Ftp,
    #[default]
    Https,
    Http,
    Rsync,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ftp => write!(f, "ftp"),
            Self::Https => write!(f, "https"),
            Self::Http => write!(f, "http"),
            Self::Rsync => write!(f, "rsync"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use itertools::Itertools;

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
    static MIRROR3: &str = r#"
            {
                "url": "http://mirror.rackspace.com/archlinux/",
                "protocol": "http",
                "last_sync": "2024-05-04T09:30:12Z",
                "completion_pct": 0.8645833333333334,
                "delay": 12205,
                "duration_avg": 0.3613546647523579,
                "duration_stddev": 0.42918278405415544,
                "score": 4.83564170785653,
                "active": true,
                "country": "",
                "country_code": "",
                "isos": true,
                "ipv4": true,
                "ipv6": false,
                "details": "https://archlinux.org/mirrors/rackspace.com/712/"
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

    #[test]
    fn sort_age() {
        let j = format!("{{\"urls\":[{MIRROR0},{MIRROR1},{MIRROR2}]}}");
        let mut ml: MirrorList = serde_json::from_str(&j).unwrap();
        ml.sort(SortKey::Age);
        // null
        assert_eq!(ml.mirrors[0].url, "https://mirrors.rutgers.edu/archlinux/");

        // 2024-04
        assert_eq!(
            ml.mirrors[1].url,
            "https://mirror.aarnet.edu.au/pub/archlinux/"
        );

        // 2024-05
        assert_eq!(ml.mirrors[2].url, "http://ftp.ntua.gr/pub/linux/archlinux/");
    }

    #[tokio::test]
    async fn update_duration() {
        let m: Mirror = serde_json::from_str(MIRROR3).unwrap();
        let m = m.update_download_rate(None).await.unwrap();
        assert!(m.download_rate.is_some());
    }

    #[tokio::test]
    async fn update_duration_large_timeout() {
        let m: Mirror = serde_json::from_str(MIRROR3).unwrap();
        let m = m
            .update_download_rate(chrono::Duration::new(20, 0))
            .await
            .unwrap();
        assert!(m.download_rate.is_some());
    }

    #[tokio::test]
    async fn update_duration_small_timeout() {
        let m: Mirror = serde_json::from_str(MIRROR3).unwrap();
        let r = m
            .clone()
            .update_download_rate(chrono::Duration::new(0, 1))
            .await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn update_duration_interrupt() {
        let m: Mirror = serde_json::from_str(MIRROR3).unwrap();
        let mut s = JoinSet::new();
        s.spawn(m.update_download_rate(None));
        s.abort_all();
    }

    #[tokio::test]
    async fn update_mirrorlist_dl_rate() {
        let mut mlist = MirrorList::from_default_url().await.unwrap();
        mlist.mirrors.truncate(15);
        let mlentgth = mlist.mirrors.len();
        mlist.update_download_rate(None, 3).await;
        mlist.sort(SortKey::Rate);
        let mirrors = &mlist.mirrors.clone();
        assert!(!&mirrors.is_empty());
        assert!(
            mirrors[0].download_rate.clone().unwrap() >= mirrors[1].download_rate.clone().unwrap()
        );
        assert_eq!(mlentgth, mlist.mirrors.len());
    }

    #[test]
    fn age_computation() {
        let j = format!("{{\"urls\":[{MIRROR0},{MIRROR1},{MIRROR2}]}}");
        let ml: MirrorList = serde_json::from_str(&j).unwrap();
        let [ref m0, ref m1, ref m2] = ml.mirrors.clone()[0..3] else {
            panic!()
        };
        assert_eq!(m0.age(), None);
        assert!(m1.age() < m2.age());
    }

    #[test]
    fn age_filter() {
        let j = format!("{{\"urls\":[{MIRROR0},{MIRROR1},{MIRROR2}]}}");
        let mut ml: MirrorList = serde_json::from_str(&j).unwrap();
        let mirror = ml.mirrors[0].clone();
        let now = Utc::now() + TimeDelta::minutes(10);
        for h in 0..20 {
            ml.mirrors.push(Mirror {
                last_sync: Some(now - TimeDelta::hours(h)),
                ..mirror.clone()
            })
        }
        let mut cur_len = ml.mirrors.len();
        assert_eq!(cur_len, 23);

        ml = ml.filter(None, false, false, false, &[]);
        assert_eq!(ml.mirrors.len(), cur_len);

        for age in (0..30).rev() {
            ml = ml.filter(Some(age as f64 * 0.7), false, false, false, &[]);
            assert!(ml.mirrors.len() <= cur_len);
            cur_len = ml.mirrors.len();
        }

        // one mirror's age is -10min
        assert_eq!(ml.mirrors.len(), 1)
    }

    #[test]
    fn flags_filter() {
        let j = format!("{{\"urls\":[{MIRROR0},{MIRROR1},{MIRROR2}]}}");
        let mut ml: MirrorList = serde_json::from_str(&j).unwrap();
        let mirror = ml.mirrors[0].clone();
        let flags = vec![None, Some(true), Some(false)]
            .into_iter()
            .combinations_with_replacement(3);
        for flag in flags {
            ml.mirrors.push(Mirror {
                isos: flag[0],
                ipv4: flag[1],
                ipv6: flag[2],
                ..mirror.clone()
            })
        }
        let cur_len = ml.mirrors.len();
        let ml_iso = ml.clone().filter(None, true, false, false, &[]);
        assert!(ml_iso.mirrors.iter().all(|m| m.isos.unwrap_or(false)));

        let ml_ip4 = ml.clone().filter(None, false, true, false, &[]);
        assert!(ml_ip4.mirrors.iter().all(|m| m.ipv4.unwrap_or(false)));

        let ml_ip6 = ml.clone().filter(None, false, false, true, &[]);
        assert!(ml_ip6.mirrors.iter().all(|m| m.ipv6.unwrap_or(false)));

        for proto in [
            vec![Protocol::Rsync, Protocol::Https],
            vec![Protocol::Http],
            vec![Protocol::Https],
        ] {
            let ml_proto = ml.clone().filter(None, true, true, true, &proto);
            assert!(ml_proto.mirrors.len() < cur_len);
            assert!(!ml_proto.mirrors.is_empty());
            assert!(ml_proto.mirrors.iter().all(|m| proto.contains(&m.protocol)));
        }

        ml = ml.filter(None, true, true, true, &[]);
        assert!(ml
            .mirrors
            .iter()
            .all(|m| m.isos.unwrap_or(false) & m.ipv4.unwrap_or(false) & m.ipv6.unwrap_or(false)));
        assert!(ml.mirrors.len() < cur_len);
        assert!(!ml.mirrors.is_empty());
    }
}
