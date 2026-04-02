// zzz_core/src/binary_data_process/z_bit.rs

use core::iter::FusedIterator;


#[derive(Clone, Debug)]
pub struct PayloadBitIter<'a> {
    data: &'a [u8],
    start_bit: usize,
    end_bit: usize,
}

impl<'a> PayloadBitIter<'a> {
    /// 按整个字节数组遍历所有位（LSB-first，每个字节内从低位到高位）
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            start_bit: 0,
            end_bit: data.len().saturating_mul(8),
        }
    }

    #[inline(always)]
    fn remaining_bits(&self) -> usize {
        self.end_bit.saturating_sub(self.start_bit)
    }
}

impl<'a> Iterator for PayloadBitIter<'a> {
    type Item = bool;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start_bit >= self.end_bit {
            return None;
        }

        let bit_pos = self.start_bit;
        let byte_idx = bit_pos / 8;
        let bit_idx = (bit_pos % 8) as u8;
        self.start_bit += 1;

        Some((self.data[byte_idx] >> bit_idx) & 1 != 0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.remaining_bits();
        (n, Some(n))
    }
}

impl<'a> ExactSizeIterator for PayloadBitIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining_bits()
    }
}

impl<'a> FusedIterator for PayloadBitIter<'a> {}




#[derive(Clone, Debug, Default)]
pub struct PayloadBitBuilder {
    data: Vec<u8>,
    bit_len: usize,
}

impl PayloadBitBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            data: Vec::with_capacity(bytes),
            bit_len: 0,
        }
    }

    #[inline(always)]
    pub fn put(&mut self, bit: bool) {
        let byte_idx = self.bit_len / 8;
        let bit_idx = (self.bit_len % 8) as u8;

        if bit_idx == 0 {
            self.data.push(0);
        }

        if bit {
            self.data[byte_idx] |= 1u8 << bit_idx;
        }

        self.bit_len += 1;
    }

    #[inline]
    pub fn put_bits(&mut self, bits: &[bool]) {
        for &bit in bits {
            self.put(bit);
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bit_len == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
    pub fn finish(self) -> Vec<u8> {
        self.data
    }
}