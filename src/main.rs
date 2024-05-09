use chrono::Duration;
use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// retrieve, filter, sort a list of the lastest Arch Linux mirrors
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = 5)]
    /// Number of seconds to wait before a download times out
    download_timeout: i64,

    #[arg(long, action)]
    /// Display a table of the distribution of server by country
    list_countries: bool,

    #[arg(long, default_value_t=reflecto::MIRROR_STATUS_URL.into())]
    /// The URL from which to retrieve the mirror date in JSON format
    url: String,

    #[arg(short, long, default_value_t=reflecto::SortKey::Score)]
    sort: reflecto::SortKey,

    #[arg(short, long, default_value_t=usize::MAX)]
    number: usize,

    #[arg(long)]
    /// If provided, where to save. otherwise, output on stdin
    save: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let mut mlist = reflecto::MirrorList::from_url(&args.url).await.unwrap();
    if args.list_countries {
        println!("{}", mlist.print_countries());
        return;
    }
    match args.sort {
        reflecto::SortKey::Rate => {
            let timeout = Duration::seconds(args.download_timeout);
            let _ = mlist.update_download_rate(Some(timeout), args.number).await;
        }
        _ => {}
    }
    mlist.sort(args.sort);
    let content = mlist.to_file_content(args.number);
    if let Some(fp) = args.save {
        let mut file = File::create(fp).expect("unable to create file");
        let _ = file.write_all(&content.into_bytes());
    } else {
        println!("{}", content);
    }
}
