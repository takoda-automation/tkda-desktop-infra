fn main() {
    if let Err(error) = tkda_desktop_tool::cloudflare::run_from_env() {
        eprintln!("tkda-cloudflare: {error}");
        std::process::exit(2);
    }
}
