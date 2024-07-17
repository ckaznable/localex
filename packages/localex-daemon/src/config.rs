#[derive(Clone, Copy)]
pub struct Config<'a> {
    pub new_profile: bool,
    pub no_save: bool,
    pub log_file_name: &'a str,
}

impl<'a> From<crate::cli::Cli> for Config<'a> {
    fn from(value: crate::cli::Cli) -> Self {
        Self {
            new_profile: value.new_profile,
            no_save: value.no_save,
            log_file_name: value.log_file_name.map(|s| s.as_ref()).unwrap_or_else(|| "runtime-log"),
        }
    }
}
