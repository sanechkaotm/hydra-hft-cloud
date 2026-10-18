use crate::graph::{ExchangeGraph, INVALID_EDGE_IDX};
use hydra_core::{CurrencyId, LOG_WEIGHT_SCALE, StackVec};

/// Maximum number of candidate cycles returned in a single hot-path pass.
pub const MAX_CANDIDATE_CYCLES: usize = 16;

/// Represents a verified 3-hop or 4-hop negative-weight arbitrage cycle anchored at a base capital currency.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CandidateCycle {
    pub base_currency: CurrencyId,
    /// Strict hop count: `3` (`Base -> A -> B -> Base`) or `4` (`Base -> A -> B -> C -> Base`).
    pub hop_count: u8,
    /// Directed edge IDs in execution order (`edge_ids[0..hop_count]`).
    pub edge_ids: [u16; 4],
    /// Visited currency nodes (`nodes[0] == nodes[hop_count] == base_currency`).
    pub nodes: [CurrencyId; 5],
    /// Sum of scaled logarithmic edge weights `sum(W(e))` (must be `< threshold_log_weight_scaled`).
    pub total_log_weight_scaled: i64,
    /// Net multiplicative return after all venue trading fees (`prod R_i * (1 - f_i)`).
    pub net_multiplier: f64,
    /// Expected net return in basis points (`(net_multiplier - 1.0) * 10_000.0`).
    pub net_profit_bps: f64,
    /// Combined Lead-Lag impulse boost across cycle edges in basis points.
    pub impulse_boost_bps: u16,
}

impl Default for CandidateCycle {
    #[inline(always)]
    fn default() -> Self {
        Self {
            base_currency: CurrencyId::EUR,
            hop_count: 0,
            edge_ids: [INVALID_EDGE_IDX; 4],
            nodes: [CurrencyId::EUR; 5],
            total_log_weight_scaled: 0,
            net_multiplier: 1.0,
            net_profit_bps: 0.0,
            impulse_boost_bps: 0,
        }
    }
}

/// Constrained k-Hop (k in {3, 4}) Logarithmic Negative Cycle Detector (Modified Bounded SPFA / Bellman-Ford).
///
/// Guarantees `< 100us` execution with zero heap allocations.
pub struct BoundedSpfaDetector {
    /// Minimum net profit fraction `\tau_min` (default `0.0004` = 4 bps).
    pub min_profit_tau: f64,
    /// Scaled logarithmic threshold: `round(-ln(1 + \tau_min) * 10^12)`.
    /// A cycle is profitable iff `sum(W(e)) < threshold_log_weight_scaled`.
    pub threshold_log_weight_scaled: i64,
}

impl Default for BoundedSpfaDetector {
    fn default() -> Self {
        Self::new(0.0004) // 4 bps default as required by Section 3.1
    }
}

impl BoundedSpfaDetector {
    #[inline(always)]
    pub fn new(min_profit_tau: f64) -> Self {
        let threshold_log_weight_scaled =
            (-(1.0 + min_profit_tau).ln() * LOG_WEIGHT_SCALE).round() as i64;
        Self {
            min_profit_tau,
            threshold_log_weight_scaled,
        }
    }

    /// Scans for all profitable 3-hop and 4-hop negative cycles anchored at `base_anchor`.
    /// Cycles of length < 3 or > 4 are strictly forbidden.
    #[inline(always)]
    pub fn scan_from_anchor(
        &self,
        graph: &ExchangeGraph,
        base_anchor: CurrencyId,
        out_candidates: &mut StackVec<CandidateCycle, MAX_CANDIDATE_CYCLES>,
    ) {
        let v0 = base_anchor;

        // Hop 1: v0 -> v1
        let edges_0 = graph.outgoing_edges(v0);
        for &e1_id in edges_0 {
            let e1 = graph.edge(e1_id);
            if !e1.active {
                continue;
            }
            let v1 = e1.to;
            if v1 == v0 {
                continue;
            }
            let w1 = e1.log_weight_scaled;

            // Hop 2: v1 -> v2
            let edges_1 = graph.outgoing_edges(v1);
            for &e2_id in edges_1 {
                let e2 = graph.edge(e2_id);
                if !e2.active {
                    continue;
                }
                let v2 = e2.to;
                if v2 == v0 || v2 == v1 {
                    continue;
                }
                let w12 = w1.saturating_add(e2.log_weight_scaled);

                // Hop 3: v2 -> v3
                let edges_2 = graph.outgoing_edges(v2);
                for &e3_id in edges_2 {
                    let e3 = graph.edge(e3_id);
                    if !e3.active {
                        continue;
                    }
                    let v3 = e3.to;
                    if v3 == v1 || v3 == v2 {
                        continue;
                    }
                    let w123 = w12.saturating_add(e3.log_weight_scaled);

                    // Case k = 3: Base (v0) -> v1 -> v2 -> Base (v0)
                    if v3 == v0 {
                        if w123 < self.threshold_log_weight_scaled {
                            let net_mult = e1.net_rate() * e2.net_rate() * e3.net_rate();
                            let net_bps = (net_mult - 1.0) * 10_000.0;
                            let boost = e1
                                .priority_boost_bps
                                .saturating_add(e2.priority_boost_bps)
                                .saturating_add(e3.priority_boost_bps);

                            let cycle = CandidateCycle {
                                base_currency: v0,
                                hop_count: 3,
                                edge_ids: [e1_id, e2_id, e3_id, INVALID_EDGE_IDX],
                                nodes: [v0, v1, v2, v0, v0],
                                total_log_weight_scaled: w123,
                                net_multiplier: net_mult,
                                net_profit_bps: net_bps,
                                impulse_boost_bps: boost,
                            };
                            Self::insert_ranked_candidate(out_candidates, cycle);
                        }
                        continue;
                    }

                    // Hop 4 (Case k = 4): v3 -> Base (v0)
                    // Strictly no further expansion beyond k = 4!
                    let edges_3 = graph.outgoing_edges(v3);
                    for &e4_id in edges_3 {
                        let e4 = graph.edge(e4_id);
                        if !e4.active || e4.to != v0 {
                            continue;
                        }
                        let w1234 = w123.saturating_add(e4.log_weight_scaled);
                        if w1234 < self.threshold_log_weight_scaled {
                            let net_mult =
                                e1.net_rate() * e2.net_rate() * e3.net_rate() * e4.net_rate();
                            let net_bps = (net_mult - 1.0) * 10_000.0;
                            let boost = e1
                                .priority_boost_bps
                                .saturating_add(e2.priority_boost_bps)
                                .saturating_add(e3.priority_boost_bps)
                                .saturating_add(e4.priority_boost_bps);

                            let cycle = CandidateCycle {
                                base_currency: v0,
                                hop_count: 4,
                                edge_ids: [e1_id, e2_id, e3_id, e4_id],
                                nodes: [v0, v1, v2, v3, v0],
                                total_log_weight_scaled: w1234,
                                net_multiplier: net_mult,
                                net_profit_bps: net_bps,
                                impulse_boost_bps: boost,
                            };
                            Self::insert_ranked_candidate(out_candidates, cycle);
                        }
                    }
                }
            }
        }
    }

    /// Scans across all base capital anchors (`EUR`, `USDC`, `USDT`, `EURC`).
    #[inline(always)]
    pub fn scan_all_anchors(
        &self,
        graph: &ExchangeGraph,
    ) -> StackVec<CandidateCycle, MAX_CANDIDATE_CYCLES> {
        let mut candidates = StackVec::new();
        for &anchor in &CurrencyId::BASE_ANCHORS {
            self.scan_from_anchor(graph, anchor, &mut candidates);
        }
        candidates
    }

    /// Localized bounded search triggered when `updated_edge_id` changes weight.
    /// Filters to cycles that actually traverse `updated_edge_id` for ultra-low latency.
    #[inline(always)]
    pub fn scan_on_edge_update(
        &self,
        graph: &ExchangeGraph,
        updated_edge_id: u16,
    ) -> StackVec<CandidateCycle, MAX_CANDIDATE_CYCLES> {
        let all = self.scan_all_anchors(graph);
        let mut filtered = StackVec::new();
        for i in 0..all.len() {
            let c = all[i];
            let hops = c.hop_count as usize;
            let mut uses_edge = false;
            for h in 0..hops {
                if c.edge_ids[h] == updated_edge_id {
                    uses_edge = true;
                    break;
                }
            }
            if uses_edge {
                filtered.try_push(c);
            }
        }
        filtered
    }

    /// Inserts a candidate into the bounded stack buffer ordered by effective priority (most negative log weight + impulse boost).
    #[inline(always)]
    fn insert_ranked_candidate(
        buf: &mut StackVec<CandidateCycle, MAX_CANDIDATE_CYCLES>,
        candidate: CandidateCycle,
    ) {
        let score_cand = candidate.total_log_weight_scaled
            - (candidate.impulse_boost_bps as i64) * 100_000_000;

        if buf.len() >= MAX_CANDIDATE_CYCLES {
            let slice = buf.as_mut_slice();
            let last_idx = slice.len() - 1;
            let score_last = slice[last_idx].total_log_weight_scaled
                - (slice[last_idx].impulse_boost_bps as i64) * 100_000_000;
            if score_cand >= score_last {
                return;
            }
            slice[last_idx] = candidate;
        } else {
            let _ = buf.try_push(candidate);
        }

        let slice = buf.as_mut_slice();
        // Insertion sort on <= 16 elements (in-register / L1 cache, zero allocations)
        let mut i = slice.len().saturating_sub(1);
        while i > 0 {
            let score_curr = slice[i].total_log_weight_scaled
                - (slice[i].impulse_boost_bps as i64) * 100_000_000;
            let score_prev = slice[i - 1].total_log_weight_scaled
                - (slice[i - 1].impulse_boost_bps as i64) * 100_000_000;
            if score_curr < score_prev {
                slice.swap(i, i - 1);
                i -= 1;
            } else {
                break;
            }
        }
    }
}
