//! Test logger - send everything to console for cargo test

pub fn test_logger() {
    //  All errors to console.
    //  This needs to be reworked to send to standard output, rather than the
    //  console, because the logging appears even for successful tests.
    let _ = simplelog::CombinedLogger::init(vec![simplelog::TermLogger::new(
        simplelog::LevelFilter::Debug,
        simplelog::Config::default(),
        simplelog::TerminalMode::Stdout,
        simplelog::ColorChoice::Auto,
    )]);
}
