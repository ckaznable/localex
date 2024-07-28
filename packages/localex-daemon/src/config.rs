use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub new_profile: bool,
    pub no_save: bool,
    pub log_file_name: String,
    pub sock: Option<PathBuf>,
}

impl From<crate::cli::Cli> for Config {
    fn from(value: crate::cli::Cli) -> Self {
        Self {
            new_profile: value.new_profile,
            no_save: value.no_save,
            log_file_name: value.log_file_name.unwrap_or_else(|| String::from("runtime-log")),
            sock: value.sock,
        }
    }
}
