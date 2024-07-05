use std::collections::HashMap;

use anyhow::Result;
use libp2p::identity::Keypair;
use secret_service::{EncryptionType, SecretService};

const SECRET_LABEL: &str = "LocalEx";

pub struct SecretStore<'a> {
    service: SecretService<'a>,
    attributes: HashMap<&'a str, &'a str>,
}

impl<'a> SecretStore<'a> {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            service: SecretService::connect(EncryptionType::Dh).await?,
            attributes: HashMap::from([("localex", "key")]),
        })
    }

    pub async fn get_local_key(&self) -> Option<Keypair> {
        let search_items = self.service
            .search_items(self.attributes.clone())
            .await
            .ok()?;

        let item = search_items.unlocked.first()?;
        let secret = item.get_secret().await.ok()?;
        Keypair::from_protobuf_encoding(&secret).ok()
    }

    pub async fn save_local_key(&self, keypair: &Keypair) -> Result<()> {
        let key = keypair.to_protobuf_encoding()?;
        let collection = self.service.get_default_collection().await?;

        collection.create_item(
            SECRET_LABEL,
            self.attributes.clone(),
            key.as_slice(),
            true,
            "text/plain"
        ).await?;

        Ok(())
    }
}
