//! High-speed LZ4 compression and decompression.

use lz4_flex::{compress_prepend_size, decompress_size_prepended};

/// Compresses a payload using LZ4 (with prepended 4-byte size header).
pub fn compress(data: &[u8]) -> Vec<u8> {
    compress_prepend_size(data)
}

/// Decompresses an LZ4 payload with prepended size header.
pub fn decompress(compressed: &[u8]) -> anyhow::Result<Vec<u8>> {
    decompress_size_prepended(compressed)
        .map_err(|e| anyhow::anyhow!("LZ4 decompression failed: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_roundtrip() {
        let input = vec![0x42u8; 10000];
        let compressed = compress(&input);
        assert!(compressed.len() < input.len());
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }
}
