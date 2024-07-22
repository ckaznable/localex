use anyhow::Result;
use libp2p::{identity::Keypair, Swarm};
use network::{new_swarm, LocalExBehaviour};

pub struct Service {
    swarm: Swarm<LocalExBehaviour>,
}

impl Service {
    pub fn new(local_keypair: Keypair) -> Result<Self> {
        let swarm = new_swarm(local_keypair)?;
        Ok(Self { swarm })
    }

    pub async fn listen(&mut self) -> Result<()> {
        todo!()
    }
}
