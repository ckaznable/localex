use std::collections::HashMap;

use anyhow::Result;
use libp2p::identity::Keypair;
use secret_service::{EncryptionType, SecretService};

const SECRET_LABEL: &str = "LocalEx";
const ATTR_KEY: &str = "localex";
const PRIVATE_KEY_ATTR: (&str, &str) = (ATTR_KEY, "keypair");

pub struct SecretStore<'a> {
    service: SecretService<'a>,
}

impl<'a> SecretStore<'a> {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            service: SecretService::connect(EncryptionType::Dh).await?,
        })
    }

    async fn get_secret(&self, attr: HashMap<&str, &str>) -> Option<Vec<u8>> {
        let search_items = self
            .service
            .search_items(attr)
            .await
            .ok()?;

        let item = search_items.unlocked.first()?;
        item.get_secret().await.ok()
    }

    async fn save_secret(&self, attr: HashMap<&str, &str>, secret: &[u8]) -> Result<()> {
        let collection = self.service.get_default_collection().await?;
        collection
            .create_item(
                SECRET_LABEL,
                attr,
                secret,
                true,
                "text/plain",
            )
            .await?;

        Ok(())
    }

    async fn get_secret_with_one_attr(&self, attr: (&str, &str)) -> Option<Vec<u8>> {
        self.get_secret(HashMap::from([attr])).await
    }

    async fn save_secret_with_one_attr(&self, attr: (&str, &str), secret: &[u8]) -> Result<()> {
        self.save_secret(HashMap::from([attr]), secret).await
    }

    pub async fn get_local_key(&self) -> Option<Keypair> {
        if let Some(secret) = self.get_secret_with_one_attr(PRIVATE_KEY_ATTR).await {
            Keypair::from_protobuf_encoding(&secret).ok()
        } else {
            Some(Keypair::generate_ed25519())
        }
    }

    pub async fn save_local_key(&self, keypair: &Keypair) -> Result<()> {
        let key = keypair.to_protobuf_encoding()?;
        self.save_secret_with_one_attr(PRIVATE_KEY_ATTR, &key).await
    }
}
