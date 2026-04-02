//zzz_core/src/binary_data_process/pack.rs
#![allow(dead_code)]
use rand::Rng;

const MAGIC: &[u8; 4] = b"AAAA";
const HEADER_SIZE: usize = 20;
const CRC_SIZE: usize = 4;
const MIN_BLOCK_SIZE: usize = HEADER_SIZE + CRC_SIZE;

const MAX_PAYLOAD_SIZE: u32 = u32::MAX;
const MAX_CHUNK_SIZE: u32 = u32::MAX - (HEADER_SIZE as u32 + CRC_SIZE as u32);
const MAX_DATA_SIZE: usize = isize::MAX as usize - HEADER_SIZE - CRC_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub idx: u32,
    pub total: u32,
    pub chunk_size: u32,
    pub payload_size: u32,
    pub data: Vec<u8>,
    pub crc32: u32,
}

impl Block {
    #[inline]
    pub fn new(idx: u32, total: u32, payload_size: u32, data: Vec<u8>) -> Self {
        let crc32 = Self::calculate_crc32(&data);
        let chunk_size = data.len() as u32;
        Self {
            idx,
            total,
            chunk_size,
            payload_size,
            data,
            crc32,
        }
    }

    #[inline]
    fn calculate_crc32(data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }

    
    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        
        let total_len = HEADER_SIZE
            .checked_add(self.data.len())
            .and_then(|x| x.checked_add(CRC_SIZE))?;

        
        if total_len > MAX_DATA_SIZE {
            return None;
        }

        let mut buf = vec![0u8; total_len];

        
        buf[0..4].copy_from_slice(MAGIC);
        buf[4..8].copy_from_slice(&self.idx.to_be_bytes());
        buf[8..12].copy_from_slice(&self.total.to_be_bytes());
        buf[12..16].copy_from_slice(&self.chunk_size.to_be_bytes());
        buf[16..20].copy_from_slice(&self.payload_size.to_be_bytes());

        buf[HEADER_SIZE..HEADER_SIZE + self.data.len()]
            .copy_from_slice(&self.data);

        buf[HEADER_SIZE + self.data.len()..].copy_from_slice(&self.crc32.to_be_bytes());

        Some(buf)
    }

    #[inline]
    pub fn verify(&self) -> bool {
        Self::calculate_crc32(&self.data) == self.crc32
    }
}

pub fn parse_block(raw: &[u8]) -> Option<Block> {
    
    if raw.len() < MIN_BLOCK_SIZE {
        return None;
    }

    
    if &raw[0..4] != MAGIC {
        return None;
    }

    
    let idx = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]);
    let total = u32::from_be_bytes([raw[8], raw[9], raw[10], raw[11]]);
    let chunk_size = u32::from_be_bytes([raw[12], raw[13], raw[14], raw[15]]);
    let payload_size = u32::from_be_bytes([raw[16], raw[17], raw[18], raw[19]]);

    
    if total == 0 || idx >= total {
        return None;
    }

    
    if chunk_size > MAX_CHUNK_SIZE {
        return None;
    }

    
    let expected_len = match HEADER_SIZE
        .checked_add(chunk_size as usize)
        .and_then(|x| x.checked_add(CRC_SIZE))
    {
        Some(len) => len,
        None => return None,
    };

    if raw.len() < expected_len {
        return None;
    }

    
    let data_end = HEADER_SIZE + chunk_size as usize;
    let data = raw[HEADER_SIZE..data_end].to_vec();

    
    let crc32 = u32::from_be_bytes([
        raw[data_end],
        raw[data_end + 1],
        raw[data_end + 2],
        raw[data_end + 3],
    ]);

    
    if Block::calculate_crc32(&data) != crc32 {
        return None;
    }

    
    if data.len() != chunk_size as usize {
        return None;
    }

    
    if payload_size == 0 {
        return None;
    }

    Some(Block {
        idx,
        total,
        chunk_size,
        payload_size,
        data,
        crc32,
    })
}

pub fn merge_blocks(blocks: Vec<Block>) -> Option<Vec<u8>> {
    
    if blocks.is_empty() {
        return None;
    }

    let total = blocks[0].total as usize;
    let size = blocks[0].payload_size as usize;

    
    if blocks.len() != total {
        return None;
    }

    
    if size > isize::MAX as usize {
        return None;
    }

    
    let mut result = Vec::with_capacity(size);
    let mut block_map: Vec<Option<Vec<u8>>> = vec![None; total];

    
    for block in blocks {
        let idx = block.idx as usize;

        
        if idx >= total
            || block_map[idx].is_some()
            || block.total as usize != total
            || block.payload_size as usize != size
            || !block.verify()  
        {
            return None;
        }

        block_map[idx] = Some(block.data);
    }

    
    for data_opt in block_map {
        match data_opt {
            Some(data) => {
                
                if result.len().checked_add(data.len()).is_none() {
                    return None;
                }
                result.extend_from_slice(&data);
            }
            None => return None,
        }
    }

    
    if result.len() != size {
        return None;
    }

    Some(result)
}

pub fn split_into_blocks(payload: &[u8], chunk_size: usize) -> Option<Vec<Block>> {
    
    if payload.is_empty() || chunk_size == 0 {
        return None;
    }

    
    if payload.len() > MAX_PAYLOAD_SIZE as usize {
        return None;
    }

    
    let total = payload
        .len()
        .checked_add(chunk_size - 1)?
        .checked_div(chunk_size)?;

    if total > u32::MAX as usize {
        return None;
    }

    let payload_len = payload.len() as u32;
    let total_u32 = total as u32;

    let blocks: Vec<Block> = payload
        .chunks(chunk_size)
        .enumerate()
        .map(|(i, chunk)| Block::new(i as u32, total_u32, payload_len, chunk.to_vec()))
        .collect();

    Some(blocks)
}

pub fn split_into_blocks_random(
    payload: &[u8],
    chunk_size_min: usize,
    chunk_size_max: usize,
) -> Option<Vec<Block>> {
    
    if payload.is_empty() || chunk_size_min == 0 || chunk_size_max == 0 {
        return None;
    }

    if chunk_size_min >= chunk_size_max {
        return None;
    }

    
    if payload.len() > MAX_PAYLOAD_SIZE as usize {
        return None;
    }

    
    let mut rng = rand::thread_rng();
    let mut chunk_data = Vec::new();
    let mut offset = 0;
    let payload_len = payload.len();

    while offset < payload_len {
        let remaining = payload_len - offset;

        
        let chunk_size = if remaining <= chunk_size_max {
            remaining
        } else {
            rng.gen_range(chunk_size_min..=chunk_size_max)
        };

        
        if chunk_size == 0 {
            return None;
        }

        chunk_data.push((offset, chunk_size));

        
        offset = match offset.checked_add(chunk_size) {
            Some(o) if o <= payload_len => o,
            _ => return None,
        };
    }

    let total = chunk_data.len();

    if total > u32::MAX as usize {
        return None;
    }

    let payload_len_u32 = payload_len as u32;
    let total_u32 = total as u32;

    let blocks: Vec<Block> = chunk_data
        .into_iter()
        .enumerate()
        .map(|(i, (start, size))| {
            let data = payload[start..start + size].to_vec();
            Block::new(i as u32, total_u32, payload_len_u32, data)
        })
        .collect();

    Some(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_blocks_random() {
        let payload = b"Hello, this is a test payload for random chunking!";

        let blocks = split_into_blocks_random(payload, 5, 15).unwrap();
        assert!(!blocks.is_empty());

        
        for block in &blocks {
            assert!(block.verify(), "Block CRC verification failed");
        }

        let merged = merge_blocks(blocks.clone()).unwrap();
        assert_eq!(merged, payload);

        for (i, block) in blocks.iter().enumerate() {
            assert_eq!(block.idx as usize, i);
            assert!(block.verify());
        }

        for (i, block) in blocks.iter().enumerate() {
            let size = block.chunk_size as usize;
            let is_first = i == 0;
            let is_last = i == blocks.len() - 1;

            if !is_first && !is_last {
                assert!(
                    size >= 5 && size <= 15,
                    "Middle block size out of range: {}",
                    size
                );
            }
        }
    }

    #[test]
    fn test_split_small_payload() {
        let payload = b"small";
        let blocks = split_into_blocks_random(payload, 10, 20).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].chunk_size as usize, 5);

        let merged = merge_blocks(blocks).unwrap();
        assert_eq!(merged, payload);
    }

    #[test]
    fn test_split_exact_multiple() {
        let payload = vec![0u8; 100];
        let blocks = split_into_blocks_random(&payload, 20, 30).unwrap();

        let merged = merge_blocks(blocks.clone()).unwrap();
        assert_eq!(merged, payload);

        for block in blocks {
            assert!(block.verify());
        }
    }

    #[test]
    fn test_invalid_params() {
        let payload = b"test";

        assert!(split_into_blocks_random(payload, 0, 10).is_none());
        assert!(split_into_blocks_random(payload, 10, 0).is_none());
        assert!(split_into_blocks_random(payload, 20, 10).is_none());
        assert!(split_into_blocks_random(b"", 5, 10).is_none());
    }

    #[test]
    fn test_corrupted_block_detection() {
        let payload = b"Test data for corruption";
        let mut blocks = split_into_blocks(payload, 5).unwrap();

        
        if let Some(block) = blocks.get_mut(0) {
            if !block.data.is_empty() {
                block.data[0] ^= 0xFF;
            }
        }

        
        assert!(merge_blocks(blocks).is_none());
    }

    #[test]
    fn test_parse_block_overflow() {
        
        let mut raw = vec![0u8; 100];
        raw[0..4].copy_from_slice(b"AAAA");
        raw[4..8].copy_from_slice(&0u32.to_be_bytes());        
        raw[8..12].copy_from_slice(&1u32.to_be_bytes());       
        raw[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        raw[16..20].copy_from_slice(&100u32.to_be_bytes());    

        
        assert!(parse_block(&raw).is_none());
    }

    #[test]
    fn test_merge_with_missing_blocks() {
        let payload = b"Complete test";
        let blocks = split_into_blocks(payload, 3).unwrap();

        
        let incomplete = blocks
            .into_iter()
            .filter(|b| b.idx != 1)
            .collect::<Vec<_>>();

        assert!(merge_blocks(incomplete).is_none());
    }

    #[test]
    fn test_merge_with_duplicate_blocks() {
        let payload = b"Duplicate test";
        let blocks = split_into_blocks(payload, 3).unwrap();

        if blocks.len() > 1 {
            let mut with_dup = blocks.clone();
            with_dup.push(blocks[0].clone());

            assert!(merge_blocks(with_dup).is_none());
        }
    }

    #[test]
    fn test_to_bytes_normal() {
        let payload = b"Normal test data";
        let blocks = split_into_blocks(payload, 5).unwrap();

        for block in blocks {
            let bytes = block.to_bytes();
            
            assert!(bytes.is_some(), "to_bytes should not return None");
            let bytes = bytes.unwrap();
            assert!(!bytes.is_empty(), "to_bytes should not return empty vec");
            assert_eq!(bytes.len(), HEADER_SIZE + block.data.len() + CRC_SIZE);
        }
    }

    #[test]
    fn test_to_bytes_overflow_handling() {
        
        
        let data = vec![0u8; 1000];
        let block = Block::new(0, 1, 1000, data);
        let bytes = block.to_bytes();

        
        assert!(bytes.is_some());
        let bytes = bytes.unwrap();
        assert!(!bytes.is_empty());
        assert!(bytes.len() >= MIN_BLOCK_SIZE);
    }

    #[test]
    fn test_large_payload_performance() {
        let payload = vec![0u8; 10 * 1024 * 1024];

        let start = std::time::Instant::now();
        let blocks = split_into_blocks_random(&payload, 256 * 1024, 512 * 1024).unwrap();
        let split_time = start.elapsed();
        println!("Split 10MB: {:?}", split_time);

        let start = std::time::Instant::now();
        let merged = merge_blocks(blocks).unwrap();
        let merge_time = start.elapsed();
        println!("Merge 10MB: {:?}", merge_time);

        assert_eq!(merged, payload);
    }

    #[test]
    fn test_to_bytes_performance() {
        let block = Block::new(0, 1, 1000, vec![0u8; 1000]);

        let start = std::time::Instant::now();
        for _ in 0..10000 {
            let _ = block.to_bytes();
        }
        println!("10000x to_bytes: {:?}", start.elapsed());
    }

    #[test]
    fn test_boundary_chunk_sizes() {
        let payload = b"Boundary test data";

        
        for min_size in &[1, 5, 10] {
            for max_size in &[20, 50, 100] {
                if min_size <= max_size {
                    let blocks = split_into_blocks_random(payload, *min_size, *max_size);
                    assert!(
                        blocks.is_some(),
                        "Failed for min={}, max={}",
                        min_size,
                        max_size
                    );

                    if let Some(b) = blocks {
                        let merged = merge_blocks(b).unwrap();
                        assert_eq!(merged, payload);
                    }
                }
            }
        }
    }

    #[test]
    fn test_empty_and_single_byte() {
        
        let payload = b"X";
        let blocks = split_into_blocks_random(payload, 5, 10).unwrap();
        assert_eq!(blocks.len(), 1);

        let merged = merge_blocks(blocks).unwrap();
        assert_eq!(merged, payload);
    }
}