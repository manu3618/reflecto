fn main() {
    println!("Hello, world!");
    let _mlist = reflecto::MirrorList::from_default_url();

    println!("url: https://archlinux.org/mirrors/status/json");
}
