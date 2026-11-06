//! Tile-based delta frame encoder.
//! Divides the desktop into 64x64 pixel tiles and transmits only the tiles that changed.

use serde::{Deserialize, Serialize};

pub const TILE_SIZE: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedTile {
    pub tile_x: u16,
    pub tile_y: u16,
    pub width: u16,
    pub height: u16,
    /// LZ4-compressed RGBA pixels for this tile
    pub compressed_pixels: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaFrame {
    pub width: u32,
    pub height: u32,
    pub is_keyframe: bool,
    pub tiles: Vec<ChangedTile>,
}

pub struct TileEncoder {
    width: u32,
    height: u32,
    prev_tile_hashes: Vec<u64>,
}

impl TileEncoder {
    pub fn new(width: u32, height: u32) -> Self {
        let tiles_x = (width as usize + TILE_SIZE - 1) / TILE_SIZE;
        let tiles_y = (height as usize + TILE_SIZE - 1) / TILE_SIZE;
        let num_tiles = tiles_x * tiles_y;

        Self {
            width,
            height,
            prev_tile_hashes: vec![0; num_tiles],
        }
    }

    /// Encodes a full RGBA frame into a DeltaFrame.
    /// `rgba_data` length must equal `width * height * 4`.
    pub fn encode_frame(&mut self, rgba_data: &[u8], force_keyframe: bool) -> DeltaFrame {
        let tiles_x = (self.width as usize + TILE_SIZE - 1) / TILE_SIZE;
        let tiles_y = (self.height as usize + TILE_SIZE - 1) / TILE_SIZE;
        let mut changed_tiles = Vec::new();

        let mut tile_buffer = Vec::with_capacity(TILE_SIZE * TILE_SIZE * 4);

        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let tile_idx = ty * tiles_x + tx;
                let start_x = tx * TILE_SIZE;
                let start_y = ty * TILE_SIZE;
                let tile_w = ((self.width as usize - start_x).min(TILE_SIZE)) as u16;
                let tile_h = ((self.height as usize - start_y).min(TILE_SIZE)) as u16;

                // Extract tile pixels into buffer
                tile_buffer.clear();
                for row in 0..tile_h as usize {
                    let pixel_y = start_y + row;
                    let row_start = (pixel_y * self.width as usize + start_x) * 4;
                    let row_end = row_start + (tile_w as usize * 4);
                    if row_end <= rgba_data.len() {
                        tile_buffer.extend_from_slice(&rgba_data[row_start..row_end]);
                    }
                }

                // Compute fast hash of tile pixels
                let hash = compute_fast_hash(&tile_buffer);

                if force_keyframe || self.prev_tile_hashes[tile_idx] != hash {
                    self.prev_tile_hashes[tile_idx] = hash;
                    let compressed = super::lz4::compress(&tile_buffer);
                    changed_tiles.push(ChangedTile {
                        tile_x: tx as u16,
                        tile_y: ty as u16,
                        width: tile_w,
                        height: tile_h,
                        compressed_pixels: compressed,
                    });
                }
            }
        }

        DeltaFrame {
            width: self.width,
            height: self.height,
            is_keyframe: force_keyframe,
            tiles: changed_tiles,
        }
    }
}

pub struct TileDecoder {
    width: u32,
    height: u32,
    pub frame_buffer: Vec<u8>,
}

impl TileDecoder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame_buffer: vec![0u8; width as usize * height as usize * 4],
        }
    }

    /// Resizes internal frame buffer if the remote resolution changes
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.frame_buffer = vec![0u8; width as usize * height as usize * 4];
    }

    /// Applies changed tiles onto the local frame buffer
    pub fn apply_delta(&mut self, delta: &DeltaFrame) -> anyhow::Result<()> {
        if self.width != delta.width || self.height != delta.height {
            self.resize(delta.width, delta.height);
        }

        for tile in &delta.tiles {
            // SECURITY CHECK: Validate tile dimensions and coordinates against screen boundaries
            let tile_w = tile.width as usize;
            let tile_h = tile.height as usize;
            if tile_w == 0 || tile_h == 0 || tile_w > TILE_SIZE || tile_h > TILE_SIZE {
                anyhow::bail!("Security Violation: Invalid tile dimensions {}x{}", tile_w, tile_h);
            }

            let start_x = tile.tile_x as usize * TILE_SIZE;
            let start_y = tile.tile_y as usize * TILE_SIZE;
            if start_x + tile_w > self.width as usize || start_y + tile_h > self.height as usize {
                anyhow::bail!("Security Violation: Tile out of screen bounds ({}, {})", start_x, start_y);
            }

            let decompressed = super::lz4::decompress(&tile.compressed_pixels)?;
            let expected_bytes = tile_w * tile_h * 4;
            if decompressed.len() != expected_bytes {
                anyhow::bail!(
                    "Security Violation: Decompressed tile length mismatch (got {}, expected {})",
                    decompressed.len(),
                    expected_bytes
                );
            }

            let row_bytes = tile_w * 4;
            for row in 0..tile_h {
                let src_start = row * row_bytes;
                let src_end = src_start + row_bytes;
                let dst_y = start_y + row;
                let dst_start = (dst_y * self.width as usize + start_x) * 4;
                let dst_end = dst_start + row_bytes;

                if src_end <= decompressed.len() && dst_end <= self.frame_buffer.len() {
                    self.frame_buffer[dst_start..dst_end]
                        .copy_from_slice(&decompressed[src_start..src_end]);
                }
            }
        }

        Ok(())
    }
}

/// Fast non-cryptographic FNV-1a 64-bit hash for differential frame detection
fn compute_fast_hash(data: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_encoder_decoder_roundtrip() {
        let width = 128;
        let height = 128;
        let mut encoder = TileEncoder::new(width, height);
        let mut decoder = TileDecoder::new(width, height);

        // Frame 1: solid color
        let frame1 = vec![255u8; width as usize * height as usize * 4];
        let delta1 = encoder.encode_frame(&frame1, true);
        assert!(delta1.is_keyframe);
        decoder.apply_delta(&delta1).unwrap();
        assert_eq!(decoder.frame_buffer, frame1);

        // Frame 2: no change -> 0 tiles sent!
        let delta2 = encoder.encode_frame(&frame1, false);
        assert_eq!(delta2.tiles.len(), 0);

        // Frame 3: modify one tile in the corner
        let mut frame3 = frame1.clone();
        frame3[0..64 * 4].fill(128); // change 1 row of first tile
        let delta3 = encoder.encode_frame(&frame3, false);
        assert_eq!(delta3.tiles.len(), 1); // Only 1 changed tile out of 4!
        decoder.apply_delta(&delta3).unwrap();
        assert_eq!(decoder.frame_buffer, frame3);
    }

    #[test]
    fn test_security_bounds_rejection() {
        let mut decoder = TileDecoder::new(128, 128);

        // Exploit vector 1: Tile coordinates outside screen
        let malicious_delta = DeltaFrame {
            width: 128,
            height: 128,
            is_keyframe: false,
            tiles: vec![ChangedTile {
                tile_x: 100, // Out of bounds! (100 * 64 > 128)
                tile_y: 100,
                width: 64,
                height: 64,
                compressed_pixels: super::super::lz4::compress(&vec![0u8; 64 * 64 * 4]),
            }],
        };
        assert!(decoder.apply_delta(&malicious_delta).is_err());

        // Exploit vector 2: Length mismatch / decompression bomb simulation
        let corrupted_delta = DeltaFrame {
            width: 128,
            height: 128,
            is_keyframe: false,
            tiles: vec![ChangedTile {
                tile_x: 0,
                tile_y: 0,
                width: 64,
                height: 64,
                compressed_pixels: super::super::lz4::compress(&vec![0u8; 10]), // Expected 64*64*4
            }],
        };
        assert!(decoder.apply_delta(&corrupted_delta).is_err());
    }
}
