//! Periodic maintenance background task.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use super::manager::SessionManager;

/// Spawn a background task that periodically runs maintenance.
pub fn spawn_maintenance(
    session: Arc<Mutex<SessionManager>>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let mut session = session.lock().await;
            if let Err(e) = session.run_maintenance_tick() {
                tracing::error!("Maintenance tick failed: {e}");
            }
        }
    })
}
