use std::collections::HashMap;
use tokio::sync::{watch, Mutex};
use uuid::Uuid;

#[derive(Default)]
pub struct SubAgentScheduler {
    active_runs: Mutex<HashMap<Uuid, watch::Sender<bool>>>,
}

impl SubAgentScheduler {
    pub fn new() -> Self {
        Self {
            active_runs: Mutex::new(HashMap::new()),
        }
    }

    pub async fn register(&self, run_id: Uuid) -> watch::Receiver<bool> {
        let (tx, rx) = watch::channel(false);
        let mut map = self.active_runs.lock().await;
        map.insert(run_id, tx);
        rx
    }

    pub async fn unregister(&self, run_id: &Uuid) {
        let mut map = self.active_runs.lock().await;
        map.remove(run_id);
    }

    pub async fn cancel(&self, run_id: &Uuid) -> bool {
        let map = self.active_runs.lock().await;
        if let Some(tx) = map.get(run_id) {
            let _ = tx.send(true);
            true
        } else {
            false
        }
    }
}
