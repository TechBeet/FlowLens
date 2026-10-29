//! Concurrent TLS decryption using Rayon parallel iterators.
//!
//! Provides a `ConcurrentDecryptor` that processes multiple TLS streams in
//! parallel across available CPU cores, improving throughput for high-volume
//! capture sessions.

use rayon::prelude::*;

/// A thread-safe concurrent TLS decryption engine.
///
/// Wraps a per-stream decryption function and dispatches batches of encrypted
/// payloads across Rayon worker threads. This is the entry point for parallel
/// TLS processing (Phase 5, Task 28).
#[derive(Clone)]
pub struct ConcurrentDecryptor {
    /// Number of worker threads to use (0 = Rayon default).
    workers: usize,
}

impl ConcurrentDecryptor {
    /// Create a new concurrent decryptor with the default thread pool.
    pub fn new() -> Self {
        Self { workers: 0 }
    }

    /// Create a concurrent decryptor with a specific worker count.
    pub fn with_workers(workers: usize) -> Self {
        Self { workers }
    }

    /// Decrypt a batch of encrypted TLS payloads in parallel.
    ///
    /// Each element of `inputs` is an encrypted byte slice. The returned
    /// vector has the same length; entries are `None` when decryption fails
    /// for that stream.
    pub fn decrypt_batch(&self, inputs: &[&[u8]]) -> Vec<Option<Vec<u8>>> {
        // Apply a custom thread pool if requested.
        if self.workers > 0 {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(self.workers)
                .build()
                .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());
            pool.install(|| {
                inputs
                    .par_iter()
                    .map(|data| decrypt_one(data))
                    .collect::<Vec<_>>()
            })
        } else {
            inputs
                .par_iter()
                .map(|data| decrypt_one(data))
                .collect::<Vec<_>>()
        }
    }

    /// Decrypt a single payload (sequential helper, used for testing).
    pub fn decrypt_one_sequential(data: &[u8]) -> Option<Vec<u8>> {
        decrypt_one(data)
    }

    /// Number of worker threads configured.
    pub fn workers(&self) -> usize {
        self.workers
    }
}

impl Default for ConcurrentDecryptor {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-stream decryption routine. Currently a pass-through stub that strips a
/// single-byte marker prefix; real implementations wire in rustls decryption.
fn decrypt_one(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    // Stub: drop the first byte (marker) and return the rest.
    Some(data[1..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypt_batch_matches_sequential() {
        let decryptor = ConcurrentDecryptor::new();
        let inputs: Vec<&[u8]> = vec![b"\x01hello", b"\x02world", b"\x03rust", b"\x04tls"];

        let parallel = decryptor.decrypt_batch(&inputs);
        let sequential: Vec<Option<Vec<u8>>> =
            inputs.iter().map(|d| decrypt_one(d)).collect();

        assert_eq!(parallel.len(), sequential.len());
        for (p, s) in parallel.iter().zip(sequential.iter()) {
            assert_eq!(p, s);
        }
    }

    #[test]
    fn test_decrypt_batch_empty_input() {
        let decryptor = ConcurrentDecryptor::new();
        let inputs: Vec<&[u8]> = vec![];
        let result = decryptor.decrypt_batch(&inputs);
        assert!(result.is_empty());
    }

    #[test]
    fn test_decrypt_batch_with_empty_payload() {
        let decryptor = ConcurrentDecryptor::new();
        let inputs: Vec<&[u8]> = vec![b"\x01hello", b"", b"\x02world"];
        let result = decryptor.decrypt_batch(&inputs);
        assert_eq!(result.len(), 3);
        assert!(result[0].is_some());
        assert!(result[1].is_none()); // empty -> None
        assert!(result[2].is_some());
    }

    #[test]
    fn test_with_workers() {
        let decryptor = ConcurrentDecryptor::with_workers(2);
        assert_eq!(decryptor.workers(), 2);
        let inputs: Vec<&[u8]> = vec![b"\x01a", b"\x02b"];
        let result = decryptor.decrypt_batch(&inputs);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ConcurrentDecryptor>();
        assert_sync::<ConcurrentDecryptor>();
    }

    #[test]
    fn test_concurrent_correctness_large_batch() {
        let decryptor = ConcurrentDecryptor::with_workers(4);
        let data: Vec<Vec<u8>> = (0..1000)
            .map(|i| {
                let mut v = vec![0x01];
                v.extend_from_slice(format!("payload-{}", i).as_bytes());
                v
            })
            .collect();
        let inputs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();
        let result = decryptor.decrypt_batch(&inputs);
        assert_eq!(result.len(), 1000);
        for (i, r) in result.iter().enumerate() {
            assert!(r.is_some());
            let expected = format!("payload-{}", i);
            assert_eq!(r.as_ref().unwrap(), expected.as_bytes());
        }
    }
}