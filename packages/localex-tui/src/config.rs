use std::path::PathBuf;

pub struct Config {
    pub log_file_name: String,
    pub sock: Option<PathBuf>,
}

impl From<crate::cli::Cli> for Config {
    fn from(value: crate::cli::Cli) -> Self {
        Self {
            log_file_name: value.log_file_name.unwrap_or_else(|| String::from("tui-log")),
            sock: value.sock,
        }
    }
}
