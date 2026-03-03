/// Compute cosine similarity between two unit-normalized vectors.
/// Returns a value in [-1, 1]; higher is more similar.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "embedding dimension mismatch");
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Apply temporal decay to a similarity score.
/// `days_since_access` — fractional days since the entry was last accessed.
/// Score decays as: `base_score * exp(-lambda * days)` where lambda = 0.1.
pub fn apply_temporal_decay(base_score: f32, days_since_access: f64) -> f32 {
    let lambda = 0.1_f64;
    let decay = (-lambda * days_since_access).exp() as f32;
    base_score * decay
}

/// Encode an f32 slice as little-endian bytes.
pub fn encode_embedding(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &x in v {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    bytes
}

/// Decode little-endian bytes back to an f32 vector.
pub fn decode_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}
