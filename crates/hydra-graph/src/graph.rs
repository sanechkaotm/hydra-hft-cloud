use hydra_core::{
    CurrencyId, LOG_WEIGHT_SCALE, MAX_CURRENCIES, MAX_EDGES, VenueId,
};

/// Maximum outgoing edges per currency node in the static adjacency table.
pub const MAX_OUT_DEGREE: usize = 64;

/// Sentinel value representing an invalid or unassigned edge index.
pub const INVALID_EDGE_IDX: u16 = u16::MAX;

/// Directed trading edge stored inside the pre-allocated flat `ExchangeGraph`.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DirectedEdge {
    pub edge_id: u16,
    pub from: CurrencyId,
    pub to: CurrencyId,
    pub venue: VenueId,
    /// Top-of-book effective exchange rate `R_{u -> v}` (units of `to` per 1 unit of `from`).
    pub rate: f64,
    /// Venue trading fee fraction `f_e` (e.g., `0.00075` for 7.5 bps).
    pub fee_fraction: f64,
    /// Logarithmic edge weight scaled by 10^12:
    /// `W(u -> v) = round(-ln(R_{u -> v} * (1 - f_e)) * 10^12)`
    pub log_weight_scaled: i64,
    /// Available top-of-book liquidity in `from` currency units.
    pub max_capacity_in: f64,
    /// Monotonic local receive timestamp in nanoseconds.
    pub last_update_ts_ns: u64,
    /// Lead-Lag priority boost in basis points received from Tri-Node UDP Mesh.
    pub priority_boost_bps: u16,
    pub active: bool,
}

impl Default for DirectedEdge {
    #[inline(always)]
    fn default() -> Self {
        Self {
            edge_id: INVALID_EDGE_IDX,
            from: CurrencyId::EUR,
            to: CurrencyId::USD,
            venue: VenueId::BinanceSpot,
            rate: 0.0,
            fee_fraction: 0.0,
            log_weight_scaled: i64::MAX / 8,
            max_capacity_in: 0.0,
            last_update_ts_ns: 0,
            priority_boost_bps: 0,
            active: false,
        }
    }
}

impl DirectedEdge {
    /// Computes the exact scaled logarithmic weight:
    /// `W(u -> v) = round(-ln(rate * (1.0 - fee_fraction)) * 10^12)`
    #[inline(always)]
    pub fn compute_scaled_log_weight(rate: f64, fee_fraction: f64) -> i64 {
        let net_rate = rate * (1.0 - fee_fraction);
        if net_rate <= 0.0 {
            return i64::MAX / 8;
        }
        let w = -net_rate.ln();
        (w * LOG_WEIGHT_SCALE).round() as i64
    }

    /// Net multiplicative factor `R_{u -> v} * (1 - f_e)`.
    #[inline(always)]
    pub fn net_rate(&self) -> f64 {
        self.rate * (1.0 - self.fee_fraction)
    }
}

/// Flat-array zero-allocation Exchange Graph supporting up to 128 currencies and 2,048 directed edges.
pub struct ExchangeGraph {
    pub edges: [DirectedEdge; MAX_EDGES],
    pub edge_count: usize,
    /// Outgoing edge indices for each currency `u` (`0..MAX_CURRENCIES`).
    pub adj_edges: [[u16; MAX_OUT_DEGREE]; MAX_CURRENCIES],
    /// Number of active outgoing edges for each currency `u`.
    pub adj_len: [u8; MAX_CURRENCIES],
    /// Fast lookup table `pair_venue_lookup[u][v][venue]` -> `edge_id` (`INVALID_EDGE_IDX` if not registered).
    pub lookup: [[[u16; 10]; 32]; 32],
}

impl Default for ExchangeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ExchangeGraph {
    /// Creates an empty pre-allocated `ExchangeGraph` on the stack/struct without dynamic resizing.
    pub fn new() -> Self {
        Self {
            edges: [DirectedEdge::default(); MAX_EDGES],
            edge_count: 0,
            adj_edges: [[INVALID_EDGE_IDX; MAX_OUT_DEGREE]; MAX_CURRENCIES],
            adj_len: [0u8; MAX_CURRENCIES],
            lookup: [[[INVALID_EDGE_IDX; 10]; 32]; 32],
        }
    }

    /// Inserts or updates a directed trading edge `(from -> to)` on `venue` in O(1) time with zero allocations.
    /// Returns the `edge_id` (`u16`).
    #[inline(always)]
    pub fn upsert_edge(
        &mut self,
        from: CurrencyId,
        to: CurrencyId,
        venue: VenueId,
        rate: f64,
        fee_fraction: f64,
        max_capacity_in: f64,
        timestamp_ns: u64,
    ) -> Option<u16> {
        let u = from.as_idx();
        let v = to.as_idx();
        let venue_idx = venue as usize;

        if u >= 32 || v >= 32 || venue_idx >= 10 {
            return None;
        }

        let log_weight_scaled = DirectedEdge::compute_scaled_log_weight(rate, fee_fraction);
        let existing_id = self.lookup[u][v][venue_idx];

        if existing_id != INVALID_EDGE_IDX {
            let idx = existing_id as usize;
            let edge = &mut self.edges[idx];
            edge.rate = rate;
            edge.fee_fraction = fee_fraction;
            edge.log_weight_scaled = log_weight_scaled;
            edge.max_capacity_in = max_capacity_in;
            edge.last_update_ts_ns = timestamp_ns;
            edge.active = rate > 0.0 && max_capacity_in > 0.0;
            return Some(existing_id);
        }

        if self.edge_count >= MAX_EDGES {
            return None;
        }
        let deg = self.adj_len[u] as usize;
        if deg >= MAX_OUT_DEGREE {
            return None;
        }

        let new_id = self.edge_count as u16;
        self.edges[self.edge_count] = DirectedEdge {
            edge_id: new_id,
            from,
            to,
            venue,
            rate,
            fee_fraction,
            log_weight_scaled,
            max_capacity_in,
            last_update_ts_ns: timestamp_ns,
            priority_boost_bps: 0,
            active: rate > 0.0 && max_capacity_in > 0.0,
        };

        self.adj_edges[u][deg] = new_id;
        self.adj_len[u] = (deg + 1) as u8;
        self.lookup[u][v][venue_idx] = new_id;
        self.edge_count += 1;

        Some(new_id)
    }

    /// Applies a temporary Lead-Lag impulse priority boost to all edges touching `(base, quote)`.
    #[inline(always)]
    pub fn apply_impulse_boost(&mut self, base: CurrencyId, quote: CurrencyId, magnitude_bps: u16) {
        let u = base.as_idx();
        let v = quote.as_idx();

        for idx in 0..self.edge_count {
            let edge = &mut self.edges[idx];
            if (edge.from.as_idx() == u && edge.to.as_idx() == v)
                || (edge.from.as_idx() == v && edge.to.as_idx() == u)
            {
                edge.priority_boost_bps = magnitude_bps;
            }
        }
    }

    /// Returns a slice of outgoing `edge_id`s from currency `u`.
    #[inline(always)]
    pub fn outgoing_edges(&self, u: CurrencyId) -> &[u16] {
        let idx = u.as_idx();
        let len = self.adj_len[idx] as usize;
        &self.adj_edges[idx][..len]
    }

    /// Retrieves a reference to a `DirectedEdge` by `edge_id`.
    #[inline(always)]
    pub fn edge(&self, edge_id: u16) -> &DirectedEdge {
        &self.edges[edge_id as usize]
    }
}
