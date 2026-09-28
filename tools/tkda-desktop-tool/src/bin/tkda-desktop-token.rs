fn main() {
    if let Err(error) = tkda_desktop_tool::secret::ensure_token_from_env() {
        eprintln!("tkda-desktop-token: {error}");
        std::process::exit(2);
    }
}
