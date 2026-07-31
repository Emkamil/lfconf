use lfconf::{ConfigValue, Store};
use std::sync::Arc;
use zbus::{interface, SignalContext};

pub struct LfConfInterface {
    pub store: Arc<Store>,
}

#[interface(name = "org.lfbe.lfconf")]
impl LfConfInterface {
    async fn get_value(&self, section: &str, key: &str) -> String {
        self.store
            .get(section, key)
            .map(|v| v.to_ron_string())
            .unwrap_or_default()
    }

    async fn has_value(&self, section: &str, key: &str) -> bool {
        self.store.get(section, key).is_some()
    }

    async fn get_section(&self, section: &str) -> std::collections::HashMap<String, String> {
        self.store
            .get_section(section)
            .into_iter()
            .map(|(k, v)| (k, v.to_ron_string()))
            .collect()
    }

    async fn list_sections(&self) -> Vec<String> {
        self.store.list_sections()
    }

    async fn set_value(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        section: String,
        key: String,
        value: String,
    ) -> zbus::fdo::Result<()> {
        let parsed = ConfigValue::from_ron_str(&value)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("Wrong RON source: {e}")))?;

        self.store.set(section.clone(), key.clone(), parsed);

        self.store
            .save()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Self::notify(&ctxt, &section, &key, &value)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    async fn reset_value(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        section: String,
        key: String,
    ) -> zbus::fdo::Result<()> {
        self.store.delete(&section, &key);

        self.store
            .save()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Self::notify(&ctxt, &section, &key, "")
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    #[zbus(signal)]
    pub async fn notify(
        ctxt: &SignalContext<'_>,
        section: &str,
        key: &str,
        value: &str,
    ) -> zbus::Result<()>;
}