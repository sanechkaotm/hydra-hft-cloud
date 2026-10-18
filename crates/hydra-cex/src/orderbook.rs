use hydra_core::{CurrencyId, MAX_BOOK_LEVELS, OrderBookSnapshot, PriceLevel, VenueId};
use hydra_graph::DirectedLegBook;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SequenceError {
    /// Incoming `first_update_id > expected_next_seq` -> dropped packet / sequence gap detected.
    SequenceGapDetected { expected: u64, received_first: u64 },
    /// Stale update whose `final_update_id <= current_seq`.
    StaleUpdate { current: u64, received_final: u64 },
}

/// Zero-allocation Local L2 OrderBook maintainer with strict sequence gap validation.
#[derive(Copy, Clone, Debug)]
pub struct LocalL2OrderBook {
    pub snapshot: OrderBookSnapshot,
    pub synchronized: bool,
}

impl LocalL2OrderBook {
    #[inline(always)]
    pub const fn new(venue: VenueId, base: CurrencyId, quote: CurrencyId) -> Self {
        Self {
            snapshot: OrderBookSnapshot::empty(venue, base, quote),
            synchronized: false,
        }
    }

    /// Initializes the orderbook from an authoritative L2 snapshot.
    #[inline(always)]
    pub fn apply_snapshot(
        &mut self,
        bids: &[PriceLevel],
        asks: &[PriceLevel],
        seq_no: u64,
        exchange_ts_ns: u64,
        local_recv_ts_ns: u64,
    ) {
        self.snapshot.bids = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];
        self.snapshot.asks = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];

        let b_len = bids.len().min(MAX_BOOK_LEVELS);
        self.snapshot.bids[..b_len].copy_from_slice(&bids[..b_len]);
        self.snapshot.bid_depth = b_len as u8;

        let a_len = asks.len().min(MAX_BOOK_LEVELS);
        self.snapshot.asks[..a_len].copy_from_slice(&asks[..a_len]);
        self.snapshot.ask_depth = a_len as u8;

        self.snapshot.seq_no = seq_no;
        self.snapshot.last_exchange_ts_ns = exchange_ts_ns;
        self.snapshot.local_recv_ts_ns = local_recv_ts_ns;
        self.synchronized = true;
    }

    /// Validates sequence continuity (`first_update_id <= seq_no + 1 <= final_update_id`)
    /// and applies incremental L2 price-level diffs in-place without heap allocation.
    #[inline(always)]
    pub fn apply_diff_update(
        &mut self,
        first_update_id: u64,
        final_update_id: u64,
        bid_updates: &[PriceLevel],
        ask_updates: &[PriceLevel],
        exchange_ts_ns: u64,
        local_recv_ts_ns: u64,
    ) -> Result<(), SequenceError> {
        if self.synchronized {
            if final_update_id <= self.snapshot.seq_no {
                return Err(SequenceError::StaleUpdate {
                    current: self.snapshot.seq_no,
                    received_final: final_update_id,
                });
            }
            if first_update_id > self.snapshot.seq_no + 1 {
                self.synchronized = false;
                return Err(SequenceError::SequenceGapDetected {
                    expected: self.snapshot.seq_no + 1,
                    received_first: first_update_id,
                });
            }
        }

        for &upd in bid_updates {
            self.upsert_bid_level(upd);
        }
        for &upd in ask_updates {
            self.upsert_ask_level(upd);
        }

        self.snapshot.seq_no = final_update_id;
        self.snapshot.last_exchange_ts_ns = exchange_ts_ns;
        self.snapshot.local_recv_ts_ns = local_recv_ts_ns;
        self.synchronized = true;
        Ok(())
    }

    /// Converts the local L2 book into a `DirectedLegBook` for `from -> to` water-filling sizing.
    /// - If `from == base` and `to == quote` (selling base into bids):
    ///   `rate = bid.price`, `capacity_in = bid.quantity`.
    /// - If `from == quote` and `to == base` (buying base from asks using quote):
    ///   `rate = 1.0 / ask.price`, `capacity_in = ask.price * ask.quantity`.
    #[inline(always)]
    pub fn to_directed_leg_book(&self, from: CurrencyId, fee_fraction: f64) -> DirectedLegBook {
        let mut leg = DirectedLegBook {
            rates: [0.0; MAX_BOOK_LEVELS],
            capacities_in: [0.0; MAX_BOOK_LEVELS],
            fee_fraction,
            level_count: 0,
        };

        if from == self.snapshot.base {
            let count = (self.snapshot.bid_depth as usize).min(MAX_BOOK_LEVELS);
            for i in 0..count {
                let b = self.snapshot.bids[i];
                if b.is_valid() {
                    leg.rates[leg.level_count] = b.price;
                    leg.capacities_in[leg.level_count] = b.quantity;
                    leg.level_count += 1;
                }
            }
        } else if from == self.snapshot.quote {
            let count = (self.snapshot.ask_depth as usize).min(MAX_BOOK_LEVELS);
            for i in 0..count {
                let a = self.snapshot.asks[i];
                if a.is_valid() {
                    leg.rates[leg.level_count] = 1.0 / a.price;
                    leg.capacities_in[leg.level_count] = a.price * a.quantity;
                    leg.level_count += 1;
                }
            }
        }

        leg
    }

    #[inline(always)]
    fn upsert_bid_level(&mut self, upd: PriceLevel) {
        let mut temp = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS + 1];
        let mut count = 0usize;
        let cur_len = (self.snapshot.bid_depth as usize).min(MAX_BOOK_LEVELS);
        let mut replaced = false;

        for i in 0..cur_len {
            let cur = self.snapshot.bids[i];
            if (cur.price - upd.price).abs() < 1e-12 {
                replaced = true;
                if upd.quantity > 0.0 {
                    temp[count] = upd;
                    count += 1;
                }
            } else if cur.is_valid() {
                temp[count] = cur;
                count += 1;
            }
        }
        if !replaced && upd.quantity > 0.0 && count <= MAX_BOOK_LEVELS {
            temp[count] = upd;
            count += 1;
        }

        // Sort bids descending by price
        for i in 1..count {
            let mut j = i;
            while j > 0 && temp[j].price > temp[j - 1].price {
                temp.swap(j, j - 1);
                j -= 1;
            }
        }

        let final_len = count.min(MAX_BOOK_LEVELS);
        self.snapshot.bids = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];
        self.snapshot.bids[..final_len].copy_from_slice(&temp[..final_len]);
        self.snapshot.bid_depth = final_len as u8;
    }

    #[inline(always)]
    fn upsert_ask_level(&mut self, upd: PriceLevel) {
        let mut temp = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS + 1];
        let mut count = 0usize;
        let cur_len = (self.snapshot.ask_depth as usize).min(MAX_BOOK_LEVELS);
        let mut replaced = false;

        for i in 0..cur_len {
            let cur = self.snapshot.asks[i];
            if (cur.price - upd.price).abs() < 1e-12 {
                replaced = true;
                if upd.quantity > 0.0 {
                    temp[count] = upd;
                    count += 1;
                }
            } else if cur.is_valid() {
                temp[count] = cur;
                count += 1;
            }
        }
        if !replaced && upd.quantity > 0.0 && count <= MAX_BOOK_LEVELS {
            temp[count] = upd;
            count += 1;
        }

        // Sort asks ascending by price
        for i in 1..count {
            let mut j = i;
            while j > 0 && temp[j].price < temp[j - 1].price {
                temp.swap(j, j - 1);
                j -= 1;
            }
        }

        let final_len = count.min(MAX_BOOK_LEVELS);
        self.snapshot.asks = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];
        self.snapshot.asks[..final_len].copy_from_slice(&temp[..final_len]);
        self.snapshot.ask_depth = final_len as u8;
    }
}
