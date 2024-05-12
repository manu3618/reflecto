use chrono::Duration;
use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// A port of Reflector.
///
/// This tool retrieve, filter, sort a list of the lastest Arch Linux mirrors
/// from the archlinux mirror status
/// and provide the content of the file `/etc/pacman.d/mirrorlist`.
#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Args {
    /// Number of seconds to wait before a download times out
    #[arg(long, default_value_t = 5)]
    download_timeout: i64,

    /// Display a table of the distribution of server by country
    #[arg(long, action)]
    list_countries: bool,

    /// The URL from which to retrieve the mirror date in JSON format
    #[arg(long, default_value_t=reflecto::MIRROR_STATUS_URL.into())]
    url: String,

    #[arg(short, long, default_value_t=reflecto::SortKey::Score)]
    sort: reflecto::SortKey,

    /// the number of mirrors to keep
    #[arg(short, long, default_value_t=usize::MAX)]
    number: usize,

    /// If provided, where to save. otherwise, output on stdin
    #[arg(long)]
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
