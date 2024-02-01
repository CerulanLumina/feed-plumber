use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Opts {
    /// The configuration file, by default ./feedplumber.toml
    #[arg(long, short)]
    pub config: Option<PathBuf>,

    /// The path to search for plugins, by default ./plugins
    #[arg(long, short)]
    pub directory: Option<PathBuf>,

    /// Additional plugin paths (plugin binaries directly). Multiple can be used.
    #[arg(long, short)]
    pub plugins: Vec<PathBuf>,
}
