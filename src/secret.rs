use std::collections::HashMap;

use anyhow::Result;
use libp2p::identity::Keypair;
use secret_service::{EncryptionType, SecretService};

const SECRET_LABEL: &str = "LocalEx";
const ATTR_KEY: &str = "localex";
const PRIVATE_KEY_ATTR: (&str, &str) = (ATTR_KEY, "keypair");
const PASSWORD_ATTR: (&str, &str) = (ATTR_KEY, "password");

pub struct SecretStore<'a> {
    service: SecretService<'a>,
}

impl<'a> SecretStore<'a> {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            service: SecretService::connect(EncryptionType::Dh).await?,
        })
    }

    pub async fn get_local_key(&self) -> Option<Keypair> {
        let search_items = self
            .service
            .search_items(HashMap::from([PRIVATE_KEY_ATTR]))
            .await
            .ok()?;

        let item = search_items.unlocked.first()?;
        let secret = item.get_secret().await.ok()?;
        Keypair::from_protobuf_encoding(&secret).ok()
    }

    pub async fn save_local_key(&self, keypair: &Keypair) -> Result<()> {
        let key = keypair.to_protobuf_encoding()?;
        let collection = self.service.get_default_collection().await?;

        collection
            .create_item(
                SECRET_LABEL,
                HashMap::from([PRIVATE_KEY_ATTR]),
                key.as_slice(),
                true,
                "text/plain",
            )
            .await?;

        Ok(())
    }

    pub async fn get_password(&self) -> Option<Vec<u8>> {
        let search_items = self
            .service
            .search_items(HashMap::from([PASSWORD_ATTR]))
            .await
            .ok()?;

        let item = search_items.unlocked.first()?;
        item.get_secret().await.ok()
    }

    pub async fn save_password(&self, password: &[u8]) -> Result<()> {
        let collection = self.service.get_default_collection().await?;
        collection
            .create_item(
                SECRET_LABEL,
                HashMap::from([PASSWORD_ATTR]),
                password,
                true,
                "text/plain",
            )
            .await?;

        Ok(())
    }
}
