//! Embedding generation for semantic search.
//! Supports multiple backends: local, API, or none (fallback to text search).

use std::collections::HashMap;
use std::sync::Arc;

/// Embedding vector (typically 384 or 1536 dimensions)
pub type Embedding = Vec<f32>;

/// Trait for embedding providers
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for text
    fn embed(&self, text: &str) -> Option<Embedding>;

    /// Generate embeddings for multiple texts (batched)
    fn embed_batch(&self, texts: &[&str]) -> Vec<Option<Embedding>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Get embedding dimension
    fn dimension(&self) -> usize;

    /// Provider name
    fn name(&self) -> &str;
}

/// No-op provider (fallback to text search)
pub struct NoOpEmbedding;

impl EmbeddingProvider for NoOpEmbedding {
    fn embed(&self, _text: &str) -> Option<Embedding> {
        None
    }

    fn dimension(&self) -> usize {
        0
    }

    fn name(&self) -> &str {
        "none"
    }
}

/// Simple TF-IDF based embedding (no ML, fast, deterministic)
pub struct TfIdfEmbedding {
    vocabulary: HashMap<String, usize>,
    dimension: usize,
}

impl TfIdfEmbedding {
    pub fn new(dimension: usize) -> Self {
        Self {
            vocabulary: HashMap::new(),
            dimension,
        }
    }

    /// Build vocabulary from corpus
    pub fn fit(&mut self, texts: &[&str]) {
        let mut word_counts: HashMap<String, usize> = HashMap::new();

        for text in texts {
            for word in text.split_whitespace() {
                let word = word.to_lowercase();
                *word_counts.entry(word).or_insert(0) += 1;
            }
        }

        // Take top N words by frequency
        let mut words: Vec<_> = word_counts.into_iter().collect();
        words.sort_by(|a, b| b.1.cmp(&a.1));

        self.vocabulary = words
            .into_iter()
            .take(self.dimension)
            .enumerate()
            .map(|(i, (word, _))| (word, i))
            .collect();
    }
}

impl EmbeddingProvider for TfIdfEmbedding {
    fn embed(&self, text: &str) -> Option<Embedding> {
        let mut embedding = vec![0.0f32; self.dimension];
        let words: Vec<_> = text.split_whitespace().collect();
        let total = words.len() as f32;

        if total == 0.0 {
            return Some(embedding);
        }

        for word in words {
            let word = word.to_lowercase();
            if let Some(&idx) = self.vocabulary.get(&word) {
                embedding[idx] += 1.0 / total;
            }
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        Some(embedding)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn name(&self) -> &str {
        "tfidf"
    }
}

/// Embedding manager that handles provider selection
pub struct EmbeddingManager {
    provider: Arc<dyn EmbeddingProvider>,
}

impl EmbeddingManager {
    pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Self {
        Self { provider }
    }

    pub fn with_tfidf(dimension: usize) -> Self {
        Self {
            provider: Arc::new(TfIdfEmbedding::new(dimension)),
        }
    }

    pub fn none() -> Self {
        Self {
            provider: Arc::new(NoOpEmbedding),
        }
    }

    pub fn embed(&self, text: &str) -> Option<Embedding> {
        self.provider.embed(text)
    }

    pub fn dimension(&self) -> usize {
        self.provider.dimension()
    }

    pub fn name(&self) -> &str {
        self.provider.name()
    }
}
