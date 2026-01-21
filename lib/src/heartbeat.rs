use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc, time::Duration};

use n0_error::{Result, anyerr};
use n0_future::task::AbortOnDropHandle;
use rand::Rng;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::datum_cloud::{DatumCloudClient, LoginState};

/// Client interface for creating/renewing connector leases.
pub trait LeaseClient: Send + Sync + Debug {
    fn ensure_connector<'a>(&'a self, endpoint_id: String) -> LeaseFuture<'a, String>;
    fn renew_lease<'a>(&'a self, connector_id: String) -> LeaseFuture<'a, ()>;
}

pub type LeaseFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

#[derive(Debug, Clone)]
pub struct AuthenticatedLeaseClient<L> {
    datum: DatumCloudClient,
    inner: L,
}

impl<L> AuthenticatedLeaseClient<L> {
    pub fn new(datum: DatumCloudClient, inner: L) -> Self {
        Self { datum, inner }
    }

    async fn ensure_logged_in(&self) -> Result<()> {
        let auth_state = self.datum.auth().load_refreshed().await?;
        if auth_state.is_none() {
            return Err(anyerr!("not logged in"));
        }
        Ok(())
    }
}

impl<L> LeaseClient for AuthenticatedLeaseClient<L>
where
    L: LeaseClient,
{
    fn ensure_connector<'a>(&'a self, endpoint_id: String) -> LeaseFuture<'a, String> {
        Box::pin(async move {
            self.ensure_logged_in().await?;
            self.inner.ensure_connector(endpoint_id).await
        })
    }

    fn renew_lease<'a>(&'a self, connector_id: String) -> LeaseFuture<'a, ()> {
        Box::pin(async move {
            self.ensure_logged_in().await?;
            self.inner.renew_lease(connector_id).await
        })
    }
}

#[derive(Debug, Default)]
pub struct NoopLeaseClient;

impl LeaseClient for NoopLeaseClient {
    fn ensure_connector<'a>(&'a self, endpoint_id: String) -> LeaseFuture<'a, String> {
        Box::pin(async move { Ok(endpoint_id.to_string()) })
    }

    fn renew_lease<'a>(&'a self, _connector_id: String) -> LeaseFuture<'a, ()> {
        Box::pin(async move { Ok(()) })
    }
}

#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    pub interval: Duration,
    pub jitter: Duration,
    pub retry_delay: Duration,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            jitter: Duration::from_secs(5),
            retry_delay: Duration::from_secs(5),
        }
    }
}

#[derive(Debug)]
pub struct HeartbeatAgent {
    endpoint_id: String,
    client: Arc<dyn LeaseClient>,
    config: HeartbeatConfig,
    cancel: CancellationToken,
    login_rx: watch::Receiver<LoginState>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use n0_error::StdResultExt;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use tokio::sync::{mpsc, watch};

    #[derive(Debug, Clone)]
    struct TestLeaseClient {
        logged_in: Arc<AtomicBool>,
        ensure_calls: Arc<AtomicUsize>,
        renew_calls: Arc<AtomicUsize>,
        ensure_tx: mpsc::UnboundedSender<()>,
        renew_tx: mpsc::UnboundedSender<()>,
    }

    impl LeaseClient for TestLeaseClient {
        fn ensure_connector<'a>(&'a self, endpoint_id: String) -> LeaseFuture<'a, String> {
            let this = self.clone();
            Box::pin(async move {
                let _ = endpoint_id;
                this.ensure_calls.fetch_add(1, Ordering::SeqCst);
                this.ensure_tx.send(()).ok();
                if !this.logged_in.load(Ordering::SeqCst) {
                    return Err(anyerr!("not logged in"));
                }
                Ok("connector-1".to_string())
            })
        }

        fn renew_lease<'a>(&'a self, _connector_id: String) -> LeaseFuture<'a, ()> {
            let this = self.clone();
            Box::pin(async move {
                this.renew_calls.fetch_add(1, Ordering::SeqCst);
                this.renew_tx.send(()).ok();
                if !this.logged_in.load(Ordering::SeqCst) {
                    return Err(anyerr!("not logged in"));
                }
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn heartbeat_waits_for_login_signal() -> Result<()> {
        let logged_in = Arc::new(AtomicBool::new(false));
        let ensure_calls = Arc::new(AtomicUsize::new(0));
        let renew_calls = Arc::new(AtomicUsize::new(0));
        let (ensure_tx, mut ensure_rx) = mpsc::unbounded_channel();
        let (renew_tx, mut renew_rx) = mpsc::unbounded_channel();
        let client = TestLeaseClient {
            logged_in: logged_in.clone(),
            ensure_calls: ensure_calls.clone(),
            renew_calls: renew_calls.clone(),
            ensure_tx,
            renew_tx,
        };

        let (login_tx, login_rx) = watch::channel(LoginState::Missing);
        let config = HeartbeatConfig {
            interval: Duration::from_millis(20),
            jitter: Duration::from_millis(0),
            retry_delay: Duration::from_millis(20),
        };

        let task = HeartbeatAgent::new(
            "endpoint-1".to_string(),
            Arc::new(client),
            login_rx,
        )
        .with_config(config)
        .spawn();

        // First attempt happens immediately and fails due to missing login.
        tokio::time::timeout(Duration::from_millis(100), ensure_rx.recv())
            .await
            .anyerr()?;

        // While logged out, we should not spin aggressively.
        let no_extra = tokio::time::timeout(Duration::from_millis(50), ensure_rx.recv()).await;
        assert!(no_extra.is_err(), "unexpected extra ensure_connector call");

        // Log in and notify.
        logged_in.store(true, Ordering::SeqCst);
        login_tx.send(LoginState::Valid).ok();

        // Should retry ensure/renew promptly after login.
        tokio::time::timeout(Duration::from_millis(100), ensure_rx.recv())
            .await
            .anyerr()?;
        tokio::time::timeout(Duration::from_millis(100), renew_rx.recv())
            .await
            .anyerr()?;

        assert!(ensure_calls.load(Ordering::SeqCst) >= 2);
        assert!(renew_calls.load(Ordering::SeqCst) >= 1);

        task.abort();
        Ok(())
    }
}
impl HeartbeatAgent {
    pub fn new(
        endpoint_id: String,
        client: Arc<dyn LeaseClient>,
        login_rx: watch::Receiver<LoginState>,
    ) -> Self {
        Self {
            endpoint_id,
            client,
            config: HeartbeatConfig::default(),
            cancel: CancellationToken::new(),
            login_rx,
        }
    }

    pub fn with_config(mut self, config: HeartbeatConfig) -> Self {
        self.config = config;
        self
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    pub fn spawn(self) -> AbortOnDropHandle<()> {
        let cancel = self.cancel.clone();
        let task = tokio::spawn(async move {
            let mut connector_id: Option<String> = None;
            let mut retry_delay = self.config.retry_delay;
            let max_retry_delay = Duration::from_secs(300);
            let mut login_rx = self.login_rx;

            fn current_login_state(login_rx: &watch::Receiver<LoginState>) -> LoginState {
                *login_rx.borrow()
            }

            async fn wait_with_login(
                cancel: &CancellationToken,
                login_rx: &mut watch::Receiver<LoginState>,
                duration: Duration,
            ) -> Option<LoginState> {
                tokio::select! {
                    _ = cancel.cancelled() => None,
                    _ = tokio::time::sleep(duration) => None,
                    res = login_rx.changed() => {
                        if res.is_ok() {
                            Some(*login_rx.borrow())
                        } else {
                            None
                        }
                    }
                }
            }

            async fn wait_for_login(
                cancel: &CancellationToken,
                login_rx: &mut watch::Receiver<LoginState>,
            ) -> Option<LoginState> {
                tokio::select! {
                    _ = cancel.cancelled() => None,
                    res = login_rx.changed() => {
                        if res.is_ok() {
                            Some(*login_rx.borrow())
                        } else {
                            None
                        }
                    }
                }
            }

            loop {
                if cancel.is_cancelled() {
                    return;
                }

                if connector_id.is_none() {
                    match self
                        .client
                        .ensure_connector(self.endpoint_id.clone())
                        .await
                    {
                        Ok(id) => {
                            connector_id = Some(id);
                            retry_delay = self.config.retry_delay;
                        }
                        Err(err) => {
                            warn!("failed to ensure connector: {err:#}");
                            if current_login_state(&login_rx) == LoginState::Missing {
                                let _ = wait_for_login(&cancel, &mut login_rx).await;
                                connector_id = None;
                                retry_delay = self.config.retry_delay;
                                continue;
                            } else if let Some(state) =
                                wait_with_login(&cancel, &mut login_rx, retry_delay).await
                            {
                                if state == LoginState::Missing {
                                    connector_id = None;
                                }
                                retry_delay = self.config.retry_delay;
                                continue;
                            }
                            retry_delay = (retry_delay * 2).min(max_retry_delay);
                            continue;
                        }
                    }
                }

                if let Some(ref id) = connector_id {
                    match self.client.renew_lease(id.clone()).await {
                        Ok(()) => {
                            debug!("connector lease renewed");
                            retry_delay = self.config.retry_delay;
                            let jitter_ms = self.config.jitter.as_millis() as u64;
                            let jitter = if jitter_ms == 0 {
                                Duration::ZERO
                            } else {
                                Duration::from_millis(rand::rng().random_range(0..=jitter_ms))
                            };
                            tokio::select! {
                                _ = cancel.cancelled() => return,
                                _ = tokio::time::sleep(self.config.interval + jitter) => {},
                            };
                        }
                        Err(err) => {
                            warn!("failed to renew lease: {err:#}");
                            if current_login_state(&login_rx) == LoginState::Missing {
                                let _ = wait_for_login(&cancel, &mut login_rx).await;
                                connector_id = None;
                                retry_delay = self.config.retry_delay;
                                continue;
                            } else if let Some(state) =
                                wait_with_login(&cancel, &mut login_rx, retry_delay).await
                            {
                                if state == LoginState::Missing {
                                    connector_id = None;
                                }
                                retry_delay = self.config.retry_delay;
                                continue;
                            }
                            retry_delay = (retry_delay * 2).min(max_retry_delay);
                        }
                    }
                }
            }
        });
        AbortOnDropHandle::new(task)
    }
}
