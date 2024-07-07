use std::collections::HashMap;

use anyhow::Result;
use libp2p::{identity::Keypair, PeerId};
use rsa::{
    pkcs1::{
        DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey
    },
    RsaPrivateKey,
    RsaPublicKey
};
use secret_service::{EncryptionType, SecretService};

const SECRET_LABEL: &str = "LocalEx";
const ATTR_KEY: &str = "localex";
const PRIVATE_KEY_ATTR: (&str, &str) = (ATTR_KEY, "keypair");
const SIGNATURE_PUB_ATTR: (&str, &str) = (ATTR_KEY, "signature_pub");
const SIGNATURE_PRI_ATTR: (&str, &str) = (ATTR_KEY, "signature_pri");
const REMOTE_PUBLIC_KEY_ATTR: (&str, &str) = (ATTR_KEY, "remote_pub_key");
const REMOTE_PEER_FIELD: &str = "remote_peer";

pub struct SecretStore<'a> {
    service: SecretService<'a>,
    remote_peer_store: HashMap<PeerId, RsaPublicKey>,
}

impl<'a> SecretStore<'a> {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            service: SecretService::connect(EncryptionType::Dh).await?,
            remote_peer_store: HashMap::new(),
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
        let secret = self.get_secret_with_one_attr(PRIVATE_KEY_ATTR).await?;
        Keypair::from_protobuf_encoding(&secret).ok().or_else(|| Some(Keypair::generate_ed25519()))
    }

    pub async fn save_local_key(&self, keypair: &Keypair) -> Result<()> {
        let key = keypair.to_protobuf_encoding()?;
        self.save_secret_with_one_attr(PRIVATE_KEY_ATTR, &key).await
    }

    pub async fn get_signed_keypair(&self) -> Option<(RsaPublicKey, RsaPrivateKey)> {
        let pubkey = self.get_secret_with_one_attr(SIGNATURE_PUB_ATTR).await; 
        let prikey = self.get_secret_with_one_attr(SIGNATURE_PRI_ATTR).await;

        pubkey.zip(prikey)
            .map(|(pubkey, prikey)| Ok((
                RsaPublicKey::from_pkcs1_der(&pubkey)?,
                RsaPrivateKey::from_pkcs1_der(&prikey)?,
            )))
            .unwrap_or_else(Self::gen_signed_keypair)
            .ok()
    }

    pub async fn save_signed_keypair(&self, public_key: &RsaPublicKey, private_key: &RsaPrivateKey) -> Result<()> {
        let pub_key = public_key.to_pkcs1_der()?;
        let pri_key = private_key.to_pkcs1_der()?;
        self.save_secret_with_one_attr(SIGNATURE_PUB_ATTR, pub_key.as_bytes()).await?;
        self.save_secret_with_one_attr(SIGNATURE_PRI_ATTR, pri_key.as_bytes()).await?;
        Ok(())
    }

    pub fn gen_signed_keypair() -> Result<(RsaPublicKey, RsaPrivateKey)> {
        let mut rng = rand::thread_rng();
        let bits = 2048;
        let private_key = RsaPrivateKey::new(&mut rng, bits)?;
        let public_key = private_key.to_public_key();
        Ok((public_key, private_key))
    }

    pub async fn get_remote_public_key(&self, peer_id: &PeerId) -> Option<&RsaPublicKey> {
        self.remote_peer_store.get(peer_id)
    }

    pub async fn save_remote_public_key(&mut self, peer_id: &PeerId, public_key: &RsaPublicKey) -> Result<()> {
        if self.remote_peer_store.contains_key(peer_id) {
            self.remote_peer_store.insert(*peer_id, public_key.clone());
            self.save_secret(
                HashMap::from([REMOTE_PUBLIC_KEY_ATTR, (REMOTE_PEER_FIELD, peer_id.to_string().as_str())]),
               public_key.to_pkcs1_der()?.as_bytes()
            ).await?;
        }

        Ok(())
    }
}
