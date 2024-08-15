use std::ops::{Deref, DerefMut};

use bit_vec::BitVec;

pub struct U32BitVec(pub BitVec<u32>);

impl Deref for U32BitVec {
    type Target = BitVec<u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for U32BitVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl U32BitVec {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(BitVec::from_elem(capacity, false))
    }

    pub fn get_zero_ranges(&self) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut start: Option<usize> = None;
        let max_length = self.0.len();

        for i in 0..max_length {
            match self.0.get(i) {
                None => break,
                Some(bit) => {
                    if bit && start.is_none() {
                        start = Some(i);
                    }

                    if let Some(s) = start {
                        ranges.push((s, i - 1));
                        start = None;
                    }
                }
            }
        }

        if let Some(s) = start {
            ranges.push((s, max_length - 1));
        }

        ranges
    }
}

