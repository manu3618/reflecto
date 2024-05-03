fn main() {
    let mlist = reflecto::MirrorList::from_default_url().unwrap();
    println!("{}", mlist.to_file_content());
}
