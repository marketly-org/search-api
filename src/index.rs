//! In-memory product search index.
//!
//! The index maps lowercase tokens -> the set of SKUs whose name contains
//! that token. Lookup is O(1) per query token followed by an O(k log k)
//! merge of the candidates by score.
//!
//! This is intentionally simple — it is the hot read path and we want
//! predictable tail latency. A real deployment would swap this for
//! OpenSearch, but for the catalog sizes we ship with (a few thousand
//! SKUs) the in-memory index is faster than any RPC.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::models::SearchHit;

/// A single product entry in the catalog.
#[derive(Debug, Clone)]
pub struct Product {
    pub sku: String,
    pub name: String,
    pub category: String,
    pub price_cents: u64,
}

pub struct SearchIndex {
    /// SKU -> product record.
    products: RwLock<HashMap<String, Product>>,
    /// token -> SKUs that contain the token in their name.
    inverted: RwLock<HashMap<String, Vec<String>>>,
}

impl SearchIndex {
    /// Build an empty index.
    pub fn new() -> Self {
        Self {
            products: RwLock::new(HashMap::new()),
            inverted: RwLock::new(HashMap::new()),
        }
    }

    /// Build an index pre-seeded with a small sample catalog so the
    /// service is usable out of the box (e.g. for smoke tests).
    pub fn with_seed_catalog() -> Self {
        let idx = Self::new();
        let seed = [
            ("SKU-1001", "Wireless Mechanical Keyboard", "peripherals", 8_999),
            ("SKU-1002", "USB-C Hub 7-in-1", "peripherals", 3_499),
            ("SKU-1003", "27 inch 4K Monitor", "displays", 32_999),
            ("SKU-1004", "Ergonomic Office Chair", "furniture", 24_999),
            ("SKU-1005", "Standing Desk Converter", "furniture", 18_499),
            ("SKU-1006", "Mechanical Keyboard Switches", "peripherals", 1_299),
            ("SKU-1007", "Wireless Mouse", "peripherals", 2_499),
            ("SKU-1008", "USB-C to HDMI Cable", "cables", 1_799),
            ("SKU-1009", "Laptop Stand Aluminum", "furniture", 4_299),
            ("SKU-1010", "Webcam 1080p", "peripherals", 5_999),
        ];
        for (sku, name, category, price) in seed {
            idx.insert(Product {
                sku: sku.to_string(),
                name: name.to_string(),
                category: category.to_string(),
                price_cents: price,
            });
        }
        idx
    }

    /// Insert (or replace) a product, updating the inverted index.
    pub fn insert(&self, p: Product) {
        let mut products = self.products.write().unwrap();
        let mut inverted = self.inverted.write().unwrap();

        // If the SKU already exists, remove its old tokens first.
        if let Some(existing) = products.get(&p.sku) {
            for tok in tokenize(&existing.name) {
                if let Some(skus) = inverted.get_mut(&tok) {
                    skus.retain(|s| s != &p.sku);
                }
            }
        }

        for tok in tokenize(&p.name) {
            inverted.entry(tok).or_default().push(p.sku.clone());
        }
        products.insert(p.sku.clone(), p);
    }

    /// Search the catalog for `query`, returning hits ranked by a simple
    /// TF-style score: one point per matched query token.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let tokens = tokenize(query);
        if tokens.is_empty() {
            return Vec::new();
        }

        let products = self.products.read().unwrap();
        let inverted = self.inverted.read().unwrap();

        // SKU -> match count
        let mut scores: HashMap<String, usize> = HashMap::new();
        for tok in &tokens {
            if let Some(skus) = inverted.get(tok) {
                for sku in skus {
                    *scores.entry(sku.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut hits: Vec<SearchHit> = scores
            .into_iter()
            .filter_map(|(sku, count)| {
                let p = products.get(&sku)?;
                Some(SearchHit {
                    sku: p.sku.clone(),
                    name: p.name.clone(),
                    category: p.category.clone(),
                    price_cents: p.price_cents,
                    score: count as f64 / tokens.len() as f64,
                })
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.sku.cmp(&b.sku))
        });
        hits
    }

    /// Number of products currently indexed.
    pub fn len(&self) -> usize {
        self.products.read().unwrap().len()
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Tokenize a string into lowercase alphanumeric terms.
fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_handles_punctuation() {
        assert_eq!(tokenize("USB-C, Hub!"), vec!["usb", "c", "hub"]);
    }

    #[test]
    fn search_returns_ranked_hits() {
        let idx = SearchIndex::with_seed_catalog();
        let hits = idx.search("mechanical keyboard");
        assert!(hits.iter().any(|h| h.sku == "SKU-1001"));
        // Switches match "mechanical keyboard" only via "mechanical".
        let switches = hits.iter().find(|h| h.sku == "SKU-1006").unwrap();
        assert!(switches.score < 1.0);
    }

    #[test]
    fn empty_query_returns_no_hits() {
        let idx = SearchIndex::with_seed_catalog();
        assert!(idx.search("").is_empty());
        assert!(idx.search("!!!").is_empty());
    }
}
