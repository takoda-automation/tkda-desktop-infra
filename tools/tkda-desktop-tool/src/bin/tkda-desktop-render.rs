fn main() {
    let result = tkda_desktop_tool::render::config_from_env()
        .and_then(|config| tkda_desktop_tool::render::write_manifest(&config));
    if let Err(error) = result {
        eprintln!("tkda-desktop-render: {error}");
        std::process::exit(2);
    }
}
