fn main() {
    if let Err(error) = tkda_desktop_tool::materialize::materialize_from_env() {
        eprintln!("tkda-desktop-materialize: {error}");
        std::process::exit(2);
    }
}
