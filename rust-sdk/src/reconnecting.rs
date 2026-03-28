//! Reconnecting bidirectional stream wrappers with exponential backoff.
//!
//! Use [`ReconnectingQuoteStreamHandle::connect`] and [`ReconnectingSwapStreamHandle::connect`]
//! when [`crate::streaming::StreamConfig::auto_reconnect`] is `true`.

use crate::error::{MarketMakerError, Result};
use crate::streaming::{
    ConnectionStats, QuoteStreamHandle, StreamConfig, SwapStats, SwapStreamHandle,
};
use crate::types::{MarketMakerQuote, MarketMakerSwap, QuoteUpdate, SwapUpdate};
use crate::MarketMakerClient;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Quote stream that reconnects on gRPC errors or clean stream end, re-syncing sequence numbers.
pub struct ReconnectingQuoteStreamHandle {
    client: MarketMakerClient,
    stream_config: StreamConfig,
    maker_id: String,
    auth_token: String,
    inner: QuoteStreamHandle,
    stats: Arc<Mutex<ConnectionStats>>,
    next_sequence: Arc<Mutex<u64>>,
    closed: Arc<AtomicBool>,
}

impl ReconnectingQuoteStreamHandle {
    /// Connect, fetch last sequence from the server, and open the quote stream.
    /// `stream_config.auto_reconnect` must be `true`.
    pub async fn connect(
        client: &mut MarketMakerClient,
        stream_config: StreamConfig,
        maker_id: String,
        auth_token: String,
    ) -> Result<Self> {
        if !stream_config.auto_reconnect {
            return Err(MarketMakerError::configuration(
                "ReconnectingQuoteStreamHandle requires StreamConfig.auto_reconnect = true",
            ));
        }

        let mut client_owned = client.clone();
        let stats = Arc::new(Mutex::new(ConnectionStats::new()));
        let last_sequence = client
            .get_last_sequence_number(maker_id.clone(), auth_token.clone())
            .await?;
        let next_sequence = last_sequence.saturating_add(1);

        let inner = client_owned
            .start_streaming_with_config_and_stats(&stream_config, stats.clone())
            .await?;

        Ok(Self {
            client: client_owned,
            stream_config,
            maker_id,
            auth_token,
            inner,
            stats,
            next_sequence: Arc::new(Mutex::new(next_sequence)),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Next quote sequence number to use (updated after each successful reconnect from the server).
    pub async fn synced_next_sequence(&self) -> u64 {
        *self.next_sequence.lock().await
    }

    /// Update the sequence counter after you publish a quote (SDK does not auto-increment sends).
    pub async fn set_next_sequence(&self, seq: u64) {
        *self.next_sequence.lock().await = seq;
    }

    async fn try_reconnect(&mut self) -> Result<()> {
        let mut delay = self.stream_config.reconnect_initial_delay;
        let max_delay = self.stream_config.reconnect_max_delay;

        for attempt in 0..self.stream_config.max_reconnect_attempts {
            info!(
                "Quote stream reconnect attempt {}/{} (waiting {:?})",
                attempt + 1,
                self.stream_config.max_reconnect_attempts,
                delay
            );
            tokio::time::sleep(delay).await;
            delay = delay.saturating_mul(2).min(max_delay);

            let last_sequence = self
                .client
                .get_last_sequence_number(self.maker_id.clone(), self.auth_token.clone())
                .await?;
            let next_sequence = last_sequence.saturating_add(1);
            *self.next_sequence.lock().await = next_sequence;

            match self
                .client
                .start_streaming_with_config_and_stats(&self.stream_config, self.stats.clone())
                .await
            {
                Ok(h) => {
                    self.inner = h;
                    let mut s = self.stats.lock().await;
                    s.reconnection();
                    info!("Quote stream reconnected; next_sequence={}", next_sequence);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Quote stream reconnect failed: {}", e);
                }
            }
        }

        Err(MarketMakerError::streaming(
            "Quote stream: max reconnection attempts exceeded",
        ))
    }

    /// Send a quote; reconnects once on send failure if configured.
    pub async fn send_quote(&mut self, quote: MarketMakerQuote) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(MarketMakerError::streaming("Stream has been closed"));
        }
        match self.inner.send_quote(quote.clone()).await {
            Ok(()) => Ok(()),
            Err(e) => {
                if self.closed.load(Ordering::Acquire) {
                    return Err(e);
                }
                warn!("Quote send failed, attempting reconnect: {}", e);
                self.try_reconnect().await?;
                self.inner.send_quote(quote).await
            }
        }
    }

    /// Receive the next server update, reconnecting transparently on stream end or gRPC error.
    pub async fn receive_update(&mut self) -> Result<Option<QuoteUpdate>> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(None);
        }
        loop {
            match self.inner.receive_update().await {
                Ok(Some(u)) => return Ok(Some(u)),
                Ok(None) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Ok(None);
                    }
                    self.try_reconnect().await?;
                }
                Err(e) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Err(e);
                    }
                    warn!("Quote receive failed, reconnecting: {}", e);
                    self.try_reconnect().await?;
                }
            }
        }
    }

    /// Receive with timeout: each wait is bounded; reconnect happens only after an end/error on the inner stream.
    pub async fn receive_update_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<QuoteUpdate>> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(None);
        }
        loop {
            match self.inner.receive_update_timeout(timeout).await {
                Ok(Some(u)) => return Ok(Some(u)),
                Ok(None) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Ok(None);
                    }
                    self.try_reconnect().await?;
                }
                Err(e) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Err(e);
                    }
                    warn!("Quote receive failed, reconnecting: {}", e);
                    self.try_reconnect().await?;
                }
            }
        }
    }

    pub async fn close(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.inner.close().await;
    }

    pub async fn close_with_timeout(&mut self, timeout: std::time::Duration) -> Result<()> {
        self.closed.store(true, Ordering::Release);
        self.inner.close_with_timeout(timeout).await
    }

    pub async fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.inner.is_closed().await
    }

    pub async fn get_stats(&self) -> ConnectionStats {
        self.stats.lock().await.clone()
    }

    pub async fn reset_stats(&self) {
        let mut s = self.stats.lock().await;
        *s = ConnectionStats::new();
    }
}

/// Swap stream that reconnects on gRPC errors or clean stream end.
pub struct ReconnectingSwapStreamHandle {
    client: MarketMakerClient,
    stream_config: StreamConfig,
    inner: SwapStreamHandle,
    stats: Arc<Mutex<SwapStats>>,
    closed: Arc<AtomicBool>,
}

impl ReconnectingSwapStreamHandle {
    /// Open a swap stream. `stream_config.auto_reconnect` must be `true`.
    pub async fn connect(
        client: &mut MarketMakerClient,
        stream_config: StreamConfig,
    ) -> Result<Self> {
        if !stream_config.auto_reconnect {
            return Err(MarketMakerError::configuration(
                "ReconnectingSwapStreamHandle requires StreamConfig.auto_reconnect = true",
            ));
        }

        let mut client_owned = client.clone();
        let stats = Arc::new(Mutex::new(SwapStats::new()));
        let inner = client_owned
            .start_swap_streaming_with_stats(stats.clone())
            .await?;

        Ok(Self {
            client: client_owned,
            stream_config,
            inner,
            stats,
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    async fn try_reconnect(&mut self) -> Result<()> {
        let mut delay = self.stream_config.reconnect_initial_delay;
        let max_delay = self.stream_config.reconnect_max_delay;

        for attempt in 0..self.stream_config.max_reconnect_attempts {
            info!(
                "Swap stream reconnect attempt {}/{} (waiting {:?})",
                attempt + 1,
                self.stream_config.max_reconnect_attempts,
                delay
            );
            tokio::time::sleep(delay).await;
            delay = delay.saturating_mul(2).min(max_delay);

            match self
                .client
                .start_swap_streaming_with_stats(self.stats.clone())
                .await
            {
                Ok(h) => {
                    self.inner = h;
                    let mut s = self.stats.lock().await;
                    s.reconnection();
                    info!("Swap stream reconnected");
                    return Ok(());
                }
                Err(e) => {
                    warn!("Swap stream reconnect failed: {}", e);
                }
            }
        }

        Err(MarketMakerError::streaming(
            "Swap stream: max reconnection attempts exceeded",
        ))
    }

    pub async fn send_swap(&mut self, swap: MarketMakerSwap) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(MarketMakerError::streaming("Stream has been closed"));
        }
        match self.inner.send_swap(swap.clone()).await {
            Ok(()) => Ok(()),
            Err(e) => {
                if self.closed.load(Ordering::Acquire) {
                    return Err(e);
                }
                warn!("Swap send failed, attempting reconnect: {}", e);
                self.try_reconnect().await?;
                self.inner.send_swap(swap).await
            }
        }
    }

    pub async fn receive_update(&mut self) -> Result<Option<SwapUpdate>> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(None);
        }
        loop {
            match self.inner.receive_update().await {
                Ok(Some(u)) => return Ok(Some(u)),
                Ok(None) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Ok(None);
                    }
                    self.try_reconnect().await?;
                }
                Err(e) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Err(e);
                    }
                    warn!("Swap receive failed, reconnecting: {}", e);
                    self.try_reconnect().await?;
                }
            }
        }
    }

    pub async fn receive_update_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<SwapUpdate>> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(None);
        }
        loop {
            match self.inner.receive_update_timeout(timeout).await {
                Ok(Some(u)) => return Ok(Some(u)),
                Ok(None) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Ok(None);
                    }
                    self.try_reconnect().await?;
                }
                Err(e) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Err(e);
                    }
                    warn!("Swap receive failed, reconnecting: {}", e);
                    self.try_reconnect().await?;
                }
            }
        }
    }

    pub async fn is_healthy(&self, config: &StreamConfig) -> bool {
        self.inner.is_healthy(config).await
    }

    pub async fn close(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.inner.close().await;
    }

    pub async fn close_with_timeout(&mut self, timeout: std::time::Duration) -> Result<()> {
        self.closed.store(true, Ordering::Release);
        self.inner.close_with_timeout(timeout).await
    }

    pub async fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.inner.is_closed().await
    }

    pub async fn get_stats(&self) -> SwapStats {
        self.stats.lock().await.clone()
    }

    pub async fn reset_stats(&self) {
        let mut s = self.stats.lock().await;
        *s = SwapStats::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_caps_at_max() {
        let mut d = std::time::Duration::from_secs(1);
        let max = std::time::Duration::from_secs(5);
        for _ in 0..10 {
            d = d.saturating_mul(2).min(max);
        }
        assert_eq!(d, max);
    }
}
