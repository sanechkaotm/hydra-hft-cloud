use crate::killswitch::HaltReason;
use hydra_core::{NodeRole, SpscQueue};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    CriticalKillSwitch,
}

/// Fixed-size zero-allocation telemetry/alert payload queued from the hot path.
#[derive(Copy, Clone, Debug)]
pub struct AlertEvent {
    pub node: NodeRole,
    pub severity: AlertSeverity,
    pub halt_reason: Option<HaltReason>,
    pub timestamp_ns: u64,
    pub msg_buf: [u8; 128],
    pub msg_len: usize,
}

impl Default for AlertEvent {
    fn default() -> Self {
        Self {
            node: NodeRole::Tokyo,
            severity: AlertSeverity::Info,
            halt_reason: None,
            timestamp_ns: 0,
            msg_buf: [0u8; 128],
            msg_len: 0,
        }
    }
}

impl AlertEvent {
    #[inline(always)]
    pub fn new(
        node: NodeRole,
        severity: AlertSeverity,
        halt_reason: Option<HaltReason>,
        timestamp_ns: u64,
        text: &str,
    ) -> Self {
        let mut msg_buf = [0u8; 128];
        let msg_len = text.len().min(128);
        msg_buf[..msg_len].copy_from_slice(&text.as_bytes()[..msg_len]);
        Self {
            node,
            severity,
            halt_reason,
            timestamp_ns,
            msg_buf,
            msg_len,
        }
    }

    #[inline(always)]
    pub fn message_str(&self) -> &str {
        std::str::from_utf8(&self.msg_buf[..self.msg_len]).unwrap_or("")
    }
}

/// Asynchronous non-blocking Telegram / Webhook telemetry reporter (`hydra-risk::alerts`).
///
/// The hot execution thread pushes `AlertEvent` items into `queue` in `< 30ns` without blocking
/// on network I/O; a background telemetry thread drains `queue` and formats HTTP POST requests.
pub struct AsyncAlertReporter {
    pub queue: SpscQueue<AlertEvent, 64>,
    pub chat_id: [u8; 32],
    pub chat_id_len: usize,
}

impl Default for AsyncAlertReporter {
    fn default() -> Self {
        Self::new("00000000")
    }
}

impl AsyncAlertReporter {
    pub fn new(telegram_chat_id: &str) -> Self {
        let mut chat_id = [0u8; 32];
        let len = telegram_chat_id.len().min(32);
        chat_id[..len].copy_from_slice(&telegram_chat_id.as_bytes()[..len]);
        Self {
            queue: SpscQueue::new(),
            chat_id,
            chat_id_len: len,
        }
    }

    /// Non-blocking hot-path enqueue (`< 30ns`, zero heap allocation).
    #[inline(always)]
    pub fn emit_nonblocking(&self, event: AlertEvent) -> bool {
        self.queue.try_push(event).is_ok()
    }

    /// Formats the next queued alert into a Telegram / Webhook JSON payload in `out_buf`.
    #[inline(always)]
    pub fn pop_and_format_json(&self, out_buf: &mut [u8; 512]) -> Option<usize> {
        let ev = self.queue.try_pop()?;
        let mut pos = 0usize;
        let mut push = |s: &[u8]| {
            let n = s.len().min(out_buf.len().saturating_sub(pos));
            out_buf[pos..pos + n].copy_from_slice(&s[..n]);
            pos += n;
        };

        push(b"{\"chat_id\":\"");
        push(&self.chat_id[..self.chat_id_len]);
        push(b"\",\"text\":\"[HYDRA-HFT ");
        push(ev.node.name().as_bytes());
        push(b"] ");
        push(ev.message_str().as_bytes());
        push(b"\"}" );
        Some(pos)
    }
}
