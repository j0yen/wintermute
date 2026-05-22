//! Composite retrieval: hybrid keyword + recency + recall-count.
//!
//! Phase 1 only does FTS5 keyword search; embeddings are a Phase 2 add-on.
//! We still re-rank with a recency / recall-count boost so the surface
//! shape matches what later phases will return.

use crate::index::{Hit, Index};
use anyhow::Result;
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct RankedHit {
    pub hit: Hit,
    pub score: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Weights {
    /// Negated BM25 contribution. `SQLite`'s `bm25()` is lower-is-better; we negate.
    pub bm25: f64,
    /// Recency contribution. Decays with days since `last_recalled_at`.
    pub recency: f64,
    /// `tanh`-squashed `recall_count`.
    pub recall_count: f64,
    /// Confidence in [0,1].
    pub confidence: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            bm25: 1.0,
            recency: 0.3,
            recall_count: 0.2,
            confidence: 0.5,
        }
    }
}

pub fn search(idx: &Index, query: &str, limit: usize) -> Result<Vec<RankedHit>> {
    search_with(idx, query, limit, Weights::default())
}

pub fn search_with(
    idx: &Index,
    query: &str,
    limit: usize,
    weights: Weights,
) -> Result<Vec<RankedHit>> {
    let raw = idx.search(query, limit * 4)?; // overfetch then re-rank
    let mut ranked: Vec<RankedHit> =
        raw.into_iter().map(|h| score(h, weights)).collect();
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(limit);
    Ok(ranked)
}

fn score(hit: Hit, w: Weights) -> RankedHit {
    let bm25_score = -hit.bm25; // higher is better
    let recency_score = match hit.last_recalled_at {
        Some(t) => {
            let days = (Utc::now() - t).num_seconds() as f64 / 86_400.0;
            (-days / 30.0).exp() // half-life ~ 21 days
        }
        None => 0.0,
    };
    let recall_score = (f64::from(hit.recall_count) / 5.0).tanh();
    let total = w.bm25 * bm25_score
        + w.recency * recency_score
        + w.recall_count * recall_score
        + w.confidence * hit.confidence;
    RankedHit { hit, score: total }
}
