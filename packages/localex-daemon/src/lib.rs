use anyhow::Result;
use daemon::Daemon;
use libp2p::identity::Keypair;
use secret::SecretStore;

pub mod behaviour;
pub mod cli;
pub mod config;

mod daemon;
mod secret;

pub async fn main(param: config::Config) -> Result<()> {
    logger::init_logger(logger::LoggerConfig {
        filename: &param.log_file_name,
        stdout: false,
    })?;

    let store = SecretStore::new().await?;
    let local_keypair = if param.new_profile {
        Keypair::generate_ed25519()
    } else {
        store
            .get_local_key()
            .await
            .expect("can't get libp2p keypair")
    };

    if !param.no_save {
        store.save_local_key(&local_keypair).await?;
    }

    let _hostname = hostname::get()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::from("unknown"));

    let mut deamon = Daemon::new(local_keypair, &_hostname)?;
    deamon.listen_on()?;
    deamon.run().await?;

    Ok(())
}
