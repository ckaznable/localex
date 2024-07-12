use clap::Parser;

#[derive(Parser, Clone, Copy)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(long)]
    pub new_profile: bool,

    #[arg(long)]
    pub no_save: bool,

    #[arg(long, short)]
    pub tui: bool,
}
