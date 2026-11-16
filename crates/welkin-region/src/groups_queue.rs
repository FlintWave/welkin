// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

pub const MAX_QUEUED_PER_AGENT: usize = 64;

/// Upper bound on repeated blocks processed from a single group packet.
pub const MAX_ENTRIES_PER_PACKET: usize = 64;

pub type Job = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>;

#[derive(Clone, Default)]
pub struct GroupTaskQueue {
    inner: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Job>>>>,
}

impl GroupTaskQueue {
    pub fn submit<F>(&self, agent: Uuid, job: F) -> bool
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut job: Job = Box::pin(job);
        let mut inner = self.inner.lock();
        let mut retries = 0;

        loop {
            let tx = match inner.get(&agent).cloned() {
                Some(tx) => tx,
                None => {
                    let (tx, mut rx) = mpsc::channel(MAX_QUEUED_PER_AGENT);
                    tokio::spawn(async move {
                        while let Some(job) = rx.recv().await {
                            job.await;
                        }
                    });
                    inner.insert(agent, tx.clone());
                    tx
                }
            };

            match tx.try_send(job) {
                Ok(()) => return true,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!(agent = %agent, "group task queue full, dropping packet");
                    return false;
                }
                Err(mpsc::error::TrySendError::Closed(returned)) => {
                    job = returned;
                    inner.remove(&agent);
                    if retries == 0 {
                        retries = 1;
                        continue;
                    }
                    return false;
                }
            }
        }
    }

    pub fn forget(&self, agent: Uuid) {
        let mut inner = self.inner.lock();
        inner.remove(&agent);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        let inner = self.inner.lock();
        inner.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::{mpsc, oneshot, Notify};
    use tokio::time::timeout;

    #[tokio::test]
    async fn ordering() {
        let q = GroupTaskQueue::default();
        let agent = Uuid::new_v4();
        let results = Arc::new(Mutex::new(Vec::new()));
        let (done_tx, done_rx) = oneshot::channel();

        for i in 0..20 {
            let results = results.clone();
            q.submit(agent, async move {
                tokio::time::sleep(Duration::from_millis(20 - i as u64)).await;
                results.lock().push(i);
            });
        }

        let done_tx = done_tx;
        q.submit(agent, async move {
            let _ = done_tx.send(());
        });

        timeout(Duration::from_secs(5), done_rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*results.lock(), (0..20).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn concurrency_across_agents() {
        let q = GroupTaskQueue::default();
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();
        let (done_tx, done_rx) = oneshot::channel();

        q.submit(agent_a, async {
            tokio::time::sleep(Duration::from_millis(300)).await;
        });
        q.submit(agent_b, async move {
            let _ = done_tx.send(());
        });

        timeout(Duration::from_millis(200), done_rx)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn cap() {
        let q = GroupTaskQueue::default();
        let agent = Uuid::new_v4();
        let notify = Arc::new(Notify::new());
        let counter = Arc::new(AtomicUsize::new(0));

        let first_notify = notify.clone();
        q.submit(agent, async move {
            first_notify.notified().await;
        });

        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;

        let mut queued = 0;
        for _ in 0..(MAX_QUEUED_PER_AGENT + 10) {
            let counter = counter.clone();
            let ok = q.submit(agent, async move {
                counter.fetch_add(1, Ordering::SeqCst);
            });
            if ok {
                queued += 1;
            }
        }
        assert_eq!(queued, MAX_QUEUED_PER_AGENT);

        notify.notify_one();

        let mut checks = 0;
        while counter.load(Ordering::SeqCst) != MAX_QUEUED_PER_AGENT && checks < 200 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            checks += 1;
        }
        assert_eq!(counter.load(Ordering::SeqCst), MAX_QUEUED_PER_AGENT);
        let _ = mpsc::channel::<()>(1);
    }

    #[tokio::test]
    async fn forget() {
        let q = GroupTaskQueue::default();
        let agent = Uuid::new_v4();
        let (done_tx, done_rx) = oneshot::channel();

        q.submit(agent, async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = done_tx.send(());
        });

        q.forget(agent);
        assert_eq!(q.len(), 0);

        timeout(Duration::from_secs(5), done_rx)
            .await
            .unwrap()
            .unwrap();

        let (done2_tx, done2_rx) = oneshot::channel();
        assert!(q.submit(agent, async move {
            let _ = done2_tx.send(());
        }));
        timeout(Duration::from_secs(5), done2_rx)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn panic_does_not_wedge() {
        let q = GroupTaskQueue::default();
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();
        let (b_tx, b_rx) = oneshot::channel();

        q.submit(agent_a, async {
            panic!("boom");
        });
        q.submit(agent_b, async move {
            let _ = b_tx.send(());
        });

        timeout(Duration::from_secs(1), b_rx).await.unwrap().unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let (a_tx, a_rx) = oneshot::channel();
        assert!(q.submit(agent_a, async move {
            let _ = a_tx.send(());
        }));
        timeout(Duration::from_secs(1), a_rx).await.unwrap().unwrap();
    }
}
