use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Zero-Latency In-Memory Nonce Tracker.
///
/// Eliminates network calls to `eth_getTransactionCount` on the critical trade execution path.
/// Provides sub-microsecond atomic nonce increments (`AtomicU64`) while allowing background
/// reconciliation with on-chain state.
pub struct LocalNonceTracker {
    current_nonce: AtomicU64,
    synced: AtomicBool,
}

impl Default for LocalNonceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalNonceTracker {
    pub const fn new() -> Self {
        Self {
            current_nonce: AtomicU64::new(0),
            synced: AtomicBool::new(false),
        }
    }

    /// Fetches the next available nonce and immediately increments the internal counter.
    /// Execution time: ~2 nanoseconds.
    #[inline(always)]
    pub fn next_nonce(&self) -> u64 {
        self.current_nonce.fetch_add(1, Ordering::SeqCst)
    }

    /// Peeks at the current nonce without incrementing.
    #[inline(always)]
    pub fn peek_nonce(&self) -> u64 {
        self.current_nonce.load(Ordering::Acquire)
    }

    /// Synchronizes the local counter with the verified on-chain nonce from RPC.
    pub fn sync_on_chain(&self, on_chain_nonce: u64) {
        let current = self.current_nonce.load(Ordering::Acquire);
        if on_chain_nonce > current || !self.synced.load(Ordering::Acquire) {
            self.current_nonce.store(on_chain_nonce, Ordering::Release);
            self.synced.store(true, Ordering::Release);
        }
    }

    /// Reverts the local counter by 1 if a local transaction failed to broadcast (recovery guard).
    pub fn rollback_once(&self) {
        let mut current = self.current_nonce.load(Ordering::Relaxed);
        while current > 0 {
            match self.current_nonce.compare_exchange_weak(
                current,
                current - 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    #[inline(always)]
    pub fn is_synced(&self) -> bool {
        self.synced.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_nonce_increments_and_rollback() {
        let tracker = LocalNonceTracker::new();
        assert_eq!(tracker.peek_nonce(), 0);
        assert!(!tracker.is_synced());

        tracker.sync_on_chain(105);
        assert!(tracker.is_synced());
        assert_eq!(tracker.peek_nonce(), 105);

        let n1 = tracker.next_nonce();
        let n2 = tracker.next_nonce();
        assert_eq!(n1, 105);
        assert_eq!(n2, 106);
        assert_eq!(tracker.peek_nonce(), 107);

        tracker.rollback_once();
        assert_eq!(tracker.peek_nonce(), 106);
    }
}
