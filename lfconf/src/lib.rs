// Copyright (C) 2026  Kamil Machowski
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use parking_lot::RwLock;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// ```no_run
/// # async fn example() -> zbus::Result<()> {
/// let config = lfconf::client::ConfigClient::connect().await?;
/// let opacity: f64 = config.get_or("appearance", "opacity", 0.95).await;
/// config.set("appearance", "opacity", 0.8).await?;
/// # Ok(()) }
/// ```
pub mod client {
    use crate::ConfigValue;
    use futures_util::StreamExt;
    use std::collections::HashMap;

    #[zbus::proxy(
        interface = "org.lfbe.lfconf",
        default_service = "org.lfbe.lfconf",
        default_path = "/org/lfbe/lfconf"
    )]
    trait LfConf {
        async fn get_value(&self, section: &str, key: &str) -> zbus::Result<String>;
        async fn has_value(&self, section: &str, key: &str) -> zbus::Result<bool>;
        async fn get_section(&self, section: &str) -> zbus::Result<HashMap<String, String>>;
        async fn set_value(&self, section: &str, key: &str, value: &str) -> zbus::Result<()>;
        async fn reset_value(&self, section: &str, key: &str) -> zbus::Result<()>;
        async fn list_sections(&self) -> zbus::Result<Vec<String>>;

        #[zbus(signal)]
        fn notify(&self, section: String, key: String, value: String) -> zbus::Result<()>;
    }

    #[derive(Debug, Clone)]
    pub struct ConfigChange {
        pub section: String,
        pub key: String,
        pub value: Option<ConfigValue>,
    }

    #[derive(Debug, Clone)]
    pub struct ConfigClient {
        proxy: LfConfProxy<'static>,
    }

    impl ConfigClient {
        #[cfg(feature = "gtk4")]
        pub fn attach_glib_watch<F>(&self, mut callback: F) -> zbus::Result<()>
        where
            F: FnMut(ConfigChange) + 'static,
        {
            let client = self.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Ok(mut stream) = client.changes().await {
                    while let Some(change) = stream.next().await {
                        callback(change);
                    }
                }
            });
            Ok(())
        }
        
        pub async fn connect() -> zbus::Result<Self> {
            let connection = zbus::Connection::session().await?;
            let proxy = LfConfProxy::new(&connection).await?;
            Ok(Self { proxy })
        }

        pub async fn get(&self, section: &str, key: &str) -> Option<ConfigValue> {
            let raw = self.proxy.get_value(section, key).await.ok()?;
            if raw.is_empty() {
                return None;
            }
            ConfigValue::from_ron_str(&raw).ok()
        }

        pub async fn get_typed<T: serde::de::DeserializeOwned>(&self, section: &str, key: &str) -> Option<T> {
            let raw = self.proxy.get_value(section, key).await.ok()?;
            if raw.is_empty() { return None; }
            ron::from_str(&raw).ok()
        }

        pub async fn get_or<T>(&self, section: &str, key: &str, default: T) -> T
        where
            T: TryFrom<ConfigValue>,
        {
            match self.get(section, key).await {
                Some(v) => T::try_from(v).unwrap_or(default),
                None => default,
            }
        }

        pub async fn set(
            &self,
            section: &str,
            key: &str,
            value: impl Into<ConfigValue>,
        ) -> zbus::Result<()> {
            self.proxy
                .set_value(section, key, &value.into().to_ron_string())
                .await
        }

        pub async fn reset(&self, section: &str, key: &str) -> zbus::Result<()> {
            self.proxy.reset_value(section, key).await
        }

        pub async fn has_value(&self, section: &str, key: &str) -> bool {
            self.proxy.has_value(section, key).await.unwrap_or(false)
        }

        pub async fn get_section(&self, section: &str) -> HashMap<String, ConfigValue> {
            self.proxy
                .get_section(section)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(k, raw)| ConfigValue::from_ron_str(&raw).ok().map(|v| (k, v)))
                .collect()
        }

        pub async fn list_sections(&self) -> zbus::Result<Vec<String>> {
            self.proxy.list_sections().await
        }

        pub async fn get_all(&self) -> HashMap<(String, String), ConfigValue> {
            let mut result = HashMap::new();
            if let Ok(sections) = self.list_sections().await {
                for section in sections {
                    for (key, value) in self.get_section(&section).await {
                        result.insert((section.clone(), key), value);
                    }
                }
            }
            result
        }

        pub async fn changes(&self) -> zbus::Result<impl futures_util::Stream<Item = ConfigChange> + '_> {
            let stream = self.proxy.receive_notify().await?;
            Ok(stream.filter_map(|signal| async move {
                let args = signal.args().ok()?;
                let value = if args.value.is_empty() {
                    None
                } else {
                    ConfigValue::from_ron_str(&args.value).ok()
                };
                Some(ConfigChange {
                    section: args.section.to_string(),
                    key: args.key.to_string(),
                    value,
                })
            }))
        }

        pub async fn watch<F>(&self, mut callback: F) -> zbus::Result<()>
        where
            F: FnMut(ConfigChange) + Send,
        {
            let mut stream = self.proxy.receive_notify().await?;
            while let Some(signal) = stream.next().await {
                let args = signal.args()?;
                let value = if args.value.is_empty() {
                    None
                } else {
                    ConfigValue::from_ron_str(&args.value).ok()
                };
                callback(ConfigChange {
                    section: args.section.to_string(),
                    key: args.key.to_string(),
                    value,
                });
            }
            Ok(())
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ConfigValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<String>),
}

impl ConfigValue {
    pub fn from_ron_str(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    pub fn to_ron_string(&self) -> String {
        ron::to_string(self).unwrap_or_default()
    }

    pub fn infer_from_str(raw: &str) -> Self {
        if let Ok(b) = raw.parse::<bool>() {
            ConfigValue::Bool(b)
        } else if let Ok(i) = raw.parse::<i64>() {
            ConfigValue::Int(i)
        } else if let Ok(f) = raw.parse::<f64>() {
            ConfigValue::Float(f)
        } else {
            ConfigValue::Str(raw.to_string())
        }
    }

    pub fn display(&self) -> String {
        match self {
            ConfigValue::Bool(b) => b.to_string(),
            ConfigValue::Int(i) => i.to_string(),
            ConfigValue::Float(f) => f.to_string(),
            ConfigValue::Str(s) => s.clone(),
            ConfigValue::List(items) => items.join(", "),
        }
    }
}

impl From<bool> for ConfigValue {
    fn from(v: bool) -> Self {
        ConfigValue::Bool(v)
    }
}
impl From<i64> for ConfigValue {
    fn from(v: i64) -> Self {
        ConfigValue::Int(v)
    }
}
impl From<f64> for ConfigValue {
    fn from(v: f64) -> Self {
        ConfigValue::Float(v)
    }
}
impl From<String> for ConfigValue {
    fn from(v: String) -> Self {
        ConfigValue::Str(v)
    }
}
impl From<&str> for ConfigValue {
    fn from(v: &str) -> Self {
        ConfigValue::Str(v.to_string())
    }
}
impl From<Vec<String>> for ConfigValue {
    fn from(v: Vec<String>) -> Self {
        ConfigValue::List(v)
    }
}

impl TryFrom<ConfigValue> for bool {
    type Error = ();
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Bool(b) => Ok(b),
            _ => Err(()),
        }
    }
}
impl TryFrom<ConfigValue> for i64 {
    type Error = ();
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Int(i) => Ok(i),
            _ => Err(()),
        }
    }
}
impl TryFrom<ConfigValue> for i32 {
    type Error = ();
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Int(i) => i32::try_from(i).map_err(|_| ()),
            _ => Err(()),
        }
    }
}
impl TryFrom<ConfigValue> for u32 {
    type Error = ();
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Int(i) => u32::try_from(i).map_err(|_| ()),
            _ => Err(()),
        }
    }
}
impl TryFrom<ConfigValue> for f64 {
    type Error = ();
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Float(f) => Ok(f),
            ConfigValue::Int(i) => Ok(i as f64),
            _ => Err(()),
        }
    }
}
impl TryFrom<ConfigValue> for String {
    type Error = ();
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Str(s) => Ok(s),
            _ => Err(()),
        }
    }
}
impl TryFrom<ConfigValue> for Vec<String> {
    type Error = ();
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::List(items) => Ok(items),
            _ => Err(()),
        }
    }
}

pub type Category = HashMap<String, ConfigValue>;

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct ConfigStorage {
    #[serde(flatten)]
    pub categories: HashMap<String, Category>,
}

pub struct Store {
    pub data: RwLock<ConfigStorage>,
    user_path: PathBuf,
}

impl Store {
    pub fn new() -> Self {
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME nie jest ustawione");
                PathBuf::from(home).join(".config")
            });

        let user_path = config_home.join("lfconf/settings.ron");
        let system_path = PathBuf::from("/usr/share/lfconf/defaults.ron");

        let mut final_config = ConfigStorage::default();

        if let Some(sys_data) = Self::load_file(&system_path) {
            final_config.categories = sys_data.categories;
        } else {
            eprintln!("Warning: brak domyślnej konfiguracji w {:?}", system_path);
        }

        if let Some(user_data) = Self::load_file(&user_path) {
            for (category_name, settings) in user_data.categories {
                let cat = final_config.categories.entry(category_name).or_default();
                for (key, value) in settings {
                    cat.insert(key, value);
                }
            }
        }

        Self {
            data: RwLock::new(final_config),
            user_path,
        }
    }

    fn load_file(path: &PathBuf) -> Option<ConfigStorage> {
        if !path.exists() {
            return None;
        }
        let content = fs::read_to_string(path).ok()?;
        ron::from_str(&content)
            .map_err(|e| eprintln!("RON Error w {:?}: {}", path, e))
            .ok()
    }

    pub fn get(&self, section: &str, key: &str) -> Option<ConfigValue> {
        let db = self.data.read();
        db.categories.get(section)?.get(key).cloned()
    }

    pub fn get_section(&self, section: &str) -> Category {
        let db = self.data.read();
        db.categories.get(section).cloned().unwrap_or_default()
    }

    pub fn list_sections(&self) -> Vec<String> {
        let db = self.data.read();
        let mut sections: Vec<String> = db.categories.keys().cloned().collect();
        sections.sort();
        sections
    }

    pub fn set(&self, section: String, key: String, value: ConfigValue) {
        let mut db = self.data.write();
        db.categories.entry(section).or_default().insert(key, value);
    }

    pub fn delete(&self, section: &str, key: &str) -> bool {
        let mut db = self.data.write();
        db.categories
            .get_mut(section)
            .map(|cat| cat.remove(key).is_some())
            .unwrap_or(false)
    }

    pub async fn save(&self) -> std::io::Result<()> {
        let ron_str = {
            let db = self.data.read();
            ron::ser::to_string_pretty(&*db, PrettyConfig::default())
                .expect("serializacja RON nie powinna zawieść")
        };

        let path = self.user_path.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let tmp_path = path.with_extension("ron.tmp");
            fs::write(&tmp_path, ron_str)?;
            fs::rename(&tmp_path, &path)
        })
        .await
        .expect("spawn_blocking task panicked")
    }
}
