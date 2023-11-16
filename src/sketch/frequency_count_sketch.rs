use std::cmp::{max, min};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// This struct maintains a 4-bit CountMinSketch [1] with periodic aging to provide the popularity
/// history for the TinyLfu admission policy [2]. The time and space efficiency of the sketch
/// allows it to cheaply estimate the frequency of an entry in a stream of cache access events.
///
/// The counter matrix is represented as a single-dimensional array holding 16 counters per slot. A
/// fixed depth of four balances the accuracy and cost, resulting in a width of four times the
/// length of the array. To retain an accurate estimation, the array's length equals the maximum
/// number of entries in the cache, increased to the closest power-of-two to exploit more efficient
/// bit masking. This configuration results in a confidence of 93.75% and an error bound of e / width.
///
/// To improve hardware efficiency, an item's counters are constrained to a 64-byte block, which is
/// the size of an L1 cache line. This differs from the theoretical ideal where counters are
/// uniformly distributed to minimize collisions. In that configuration, the memory accesses are
/// not predictable and lack spatial locality, which may cause the pipeline to need to wait for
/// four memory loads. Instead, the items are uniformly distributed to blocks, and each counter is
/// uniformly selected from a distinct 16-byte segment. While the runtime memory layout may result
/// in the blocks not being cache-aligned, the L2 spatial prefetcher tries to load aligned pairs of
/// cache lines, so the typical cost is only one memory access.
///
/// The frequency of all entries is aged periodically using a sampling window based on the maximum
/// number of entries in the cache. This is referred to as the reset operation by TinyLfu and keeps
/// the sketch fresh by dividing all counters by two and subtracting based on the number of odd
/// counters found. The O(n) cost of aging is amortized, ideal for hardware prefetching, and uses
/// inexpensive bit manipulations per array location.
///
/// [1] An Improved Data Stream Summary: The Count-Min Sketch and its Applications
/// http://dimacs.rutgers.edu/~graham/pubs/papers/cm-full.pdf
/// [2] TinyLFU: A Highly Efficient Cache Admission Policy
/// https://dl.acm.org/citation.cfm?id=3149371
/// [3] Hash Function Prospector: Three round functions
/// https://github.com/skeeto/hash-prospector#three-round-functions
#[derive(Debug)]
pub struct FrequencyCountSketch {
    // Frequency reduction threshold
    sample_size: usize,
    block_mask: usize,
    // Access frequency container
    table: Box<Vec<u64>>,
    table_len: usize,
    size: usize,
    max_size: usize,
}

impl FrequencyCountSketch {

    /// Initializes and increases the capacity of this <tt>FrequencySketch</tt> instance, if necessary,
    /// to ensure that it can accurately estimate the popularity of elements given the maximum size of
    /// the cache. This operation forgets all previous counts when resizing.
    pub fn new(maximum_size: usize) -> Self {
        // 最大值，i32 / 2
        let maximum = min(maximum_size, i32::MAX as usize >> 1);
        let mut sample_size = 10usize;
        if maximum > 0 {
            sample_size = 10 * maximum;
        }
        let table_len:usize = max(ceiling_power_of_two(maximum as i32), 8) as usize;
        Self {
            sample_size,
            block_mask: (table_len >> 3) - 1,
            table: Box::new(vec![0; table_len]),
            table_len,
            size: 0,
            max_size: maximum,
        }
    }

    /// Return max size of this sketch
    pub fn get_max_size(&self) -> usize {
        self.max_size
    }

    /// Return table len of this sketch
    pub fn get_table_len(&self) -> usize {
        self.table_len
    }

    /// Return the estimated number of occurrences of an element, up to the maximum (15).
    pub fn frequency<E: Hash>(&self, e: E) -> u8 {
        let mut count:[u8; 4] = [0; 4];
        let hash_code = default_hash_code(e);
        let block_hash = self.spread(hash_code);
        let counter_hash = self.rehash(block_hash);
        let block = (block_hash & self.block_mask) << 3;
        for i in 0..4 {
            let h = counter_hash >> (i << 3);
            let index = (h >> 1) & 15;
            let offset = h & 1;
            count[i] = ((self.table[block + offset + (i << 1)] >> (index << 2)) & 0xf) as u8;
        }
        min(min(count[0], count[1]), min(count[2], count[3]))
    }

    /// Increments the popularity of the element if it does not exceed the maximum (15). The popularity
    /// of all elements will be periodically down sampled when the observed events exceed a threshold.
    /// This process provides a frequency aging to allow expired long term entries to fade away.
    pub fn increment<E: Hash>(&mut self, e: E) {
        let mut index:[usize;8] = [0;8];
        let hash_code = default_hash_code(e);
        let block_hash = self.spread(hash_code);
        let counter_hash = self.rehash(block_hash);
        let block = (block_hash & self.block_mask) << 3;
        for i in 0..4 {
            let h = counter_hash >> (i << 3);
            index[i] = (h >> 1) & 15;
            let offset = h & 1;
            index[i + 4] = block + offset + (i << 1u64);
        }
        let added = self.increment_at(index[4], index[0])
            | self.increment_at(index[5], index[1])
            | self.increment_at(index[6], index[2])
            | self.increment_at(index[7], index[3]);

        if added {
            self.size += 1;
            if self.size == self.sample_size {
                self.reset();
            }
        }
    }

    /// Reduces every counter by half of its original value.
    pub fn reset(&mut self) {
        let mut count = 0u8;
        for i in self.table.iter_mut() {
            count += bit_count(*i & 0x1111111111111111);
            *i = *i >> 1 & 0x7777777777777777;
        }
        self.size = (self.size - (count >> 2) as usize) >> 1;
    }

    /// Increments the specified counter by 1 if it is not already at the maximum value (15).
    fn increment_at(&mut self, i: usize, j: usize) -> bool {
        let offset = (j as u64) << 2u64;
        let mask = 0xfu64 << offset;
        if (self.table[i] & mask) != mask {
            self.table[i] += 1u64 << offset;
            return true;
        }
        false
    }

    /// Applies a supplemental hash functions to defends against poor quality hash.
    fn spread(&self, hash_code: u64) -> usize {
        let mut x: u128 = hash_code as u128;
        x ^= x >> 17;
        x *= 0xed5ad4bb;
        x ^= x >> 11;
        x *= 0xac4c1b51;
        x ^= x >> 15;
        return x as usize;
    }

    /// Applies another round of hashing for additional randomization.
    fn rehash(&self, x: usize) -> usize {
        let mut x = x as u128;
        x *= 0x31848bab;
        x ^= x >> 14;
        return x as usize;
    }
}

/// Returns the number of one-bits in the two's complement binary representation of the specified long value.
/// This function is sometimes referred to as the population count.
pub fn bit_count(mut i: u64) -> u8 {
    // HD, Figure 5-2
    i = i - ((i >> 1) & 0x5555555555555555);
    i = (i & 0x3333333333333333) + ((i >> 2) & 0x3333333333333333);
    i = (i + (i >> 4)) & 0x0f0f0f0f0f0f0f0f;
    i = i + (i >> 8);
    i = i + (i >> 16);
    i = i + (i >> 32);
    (i & 0x7) as u8
}

///
pub fn default_hash_code<E: Hash>(e: E) -> u64 {
    let mut hasher = DefaultHasher::new();
    e.hash(&mut hasher);
    hasher.finish()
}

/// Returns the smallest power of two greater than or equal to num.
pub fn ceiling_power_of_two(num: i32) -> u32 {
    let a = number_of_leading_zeros(num - 1);
    if a == 32 || a == 0 {
        return 1;
    }
    let n = (-1i32 as u32) >> a;
    return if n >= (1 << 30) {
        1 << 30
    } else {
        n + 1
    };
}

/// Returns the number of zero bits preceding the highest-order ("leftmost") one-bit in the two's
/// complement binary representation of the specified int value.
/// Returns 32 if the specified value has no one-bits in its two's complement representation,
/// in other words if it is equal to zero.
pub fn number_of_leading_zeros(i: i32) -> u8 {
    if i == 0 {
        return 32;
    } else if i < 0 {
        return 0;
    }

    let mut n = 31u8;
    let mut b = i as u32;
    if b >= (1 << 16) {
        n -= 16;
        b = b >> 16;
    }
    if b >= (1 << 8) {
        n -= 8;
        b >>= 8;
    }
    if b >= (1 << 4) {
        n -= 4;
        b >>= 4;
    }
    if b >= (1 << 2) {
        n -= 2;
        b >>= 2;
    }
    n - ((b >> 1) as u8)
}