use clap::Parser;

/// retrieve, filter, sort a list of the lastest Arch Linux mirrors
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, action)]
    list_countries: bool,
    #[arg(short, long, default_value_t=reflecto::SortKey::Score)]
    sort: reflecto::SortKey,
    #[arg(short, long, default_value_t=usize::MAX)]
    number: usize,
}

fn main() {
    let args = Args::parse();
    let mut mlist = reflecto::MirrorList::from_default_url().unwrap();
    if args.list_countries {
        println!(
            "{}",
            mlist
                .get_countries()
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
        return;
    }
    mlist.sort(args.sort);
    println!("{}", mlist.to_file_content(args.number));
}
