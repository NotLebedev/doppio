use std::collections::HashMap;

use anyhow::Result;
use tokio::sync::RwLock;
use zbus::{proxy, zvariant::OwnedFd, Connection};
pub struct State<'a> {
    proxy: ManagerProxy<'a>,
    active_locks: RwLock<HashMap<String, OwnedFd>>,
}

impl<'a> State<'a> {
    pub async fn new(connection: &Connection) -> Result<Self> {
        Ok(State {
            proxy: ManagerProxy::new(&connection).await?,
            active_locks: RwLock::new(HashMap::default()),
        })
    }

    pub async fn inhibit(&self, string: &str) -> Result<()> {
        let read = self.active_locks.read().await;
        if !read.contains_key(string) {
            drop(read);

            let why = format!("Request from {}", string);
            let fd = self.proxy.inhibit("idle", "doppio", &why, "block").await?;

            let mut write = self.active_locks.write().await;

            write.insert(string.to_string(), fd);
        }

        Ok(())
    }

    pub async fn release(&self, string: &str) {
        let read = self.active_locks.read().await;
        if read.contains_key(string) {
            drop(read);

            let mut write = self.active_locks.write().await;

            write.remove(string);
        }
    }

    pub async fn is_inhibited(&self, string: &str) -> bool {
        self.active_locks.read().await.contains_key(string)
    }

    pub async fn active_inhibitors(&self) -> Vec<String> {
        self.active_locks.read().await.keys().cloned().collect()
    }
}

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait Manager {
    #[inline]
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;
}
