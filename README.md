# rust_frequency_counter

<p>
This struct maintains a 4-bit CountMinSketch [1] with periodic aging to provide the popularity
history for the TinyLfu admission policy [2]. The time and space efficiency of the sketch
allows it to cheaply estimate the frequency of an entry in a stream of cache access events.
</p>
<p>
The counter matrix is represented as a single-dimensional array holding 16 counters per slot. A
fixed depth of four balances the accuracy and cost, resulting in a width of four times the
length of the array. To retain an accurate estimation, the array's length equals the maximum
number of entries in the cache, increased to the closest power-of-two to exploit more efficient
bit masking. This configuration results in a confidence of 93.75% and an error bound of e / width.
</p>
<p>
To improve hardware efficiency, an item's counters are constrained to a 64-byte block, which is
the size of an L1 cache line. This differs from the theoretical ideal where counters are
uniformly distributed to minimize collisions. In that configuration, the memory accesses are
not predictable and lack spatial locality, which may cause the pipeline to need to wait for
four memory loads. Instead, the items are uniformly distributed to blocks, and each counter is
uniformly selected from a distinct 16-byte segment. While the runtime memory layout may result
in the blocks not being cache-aligned, the L2 spatial prefetcher tries to load aligned pairs of
cache lines, so the typical cost is only one memory access.
</p>

<p>
The frequency of all entries is aged periodically using a sampling window based on the maximum
number of entries in the cache. This is referred to as the reset operation by TinyLfu and keeps
the sketch fresh by dividing all counters by two and subtracting based on the number of odd
counters found. The O(n) cost of aging is amortized, ideal for hardware prefetching, and uses
inexpensive bit manipulations per array location.
</p>

- [1] An Improved Data Stream Summary: The Count-Min Sketch and its Applications 
- http://dimacs.rutgers.edu/~graham/pubs/papers/cm-full.pdf
- [2] TinyLFU: A Highly Efficient Cache Admission Policy
- https://dl.acm.org/citation.cfm?id=3149371
- [3] Hash Function Prospector: Three round functions
- https://github.com/skeeto/hash-prospector#three-round-functions