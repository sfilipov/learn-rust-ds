## Motivation
This repository implements basic non-production versions of an AVL tree (self balancing tree). The operations supported by the different implementations are the same. They differ in how they store their nodes.

My goal for this project was to gain more Rust experience by building something non-trivial and also touches on some of the more complicated Rust concepts like unsafe, pointers and borrowing.

I chose AVL tree as the data structure to implement because it is:
* naturally cyclic, posing a challenge with ownership
* complex enough to implement (good for practice) but not too complex
* maintains O(logn) speed on all operations which makes it suitable to later compare performance between different "storage" implementations and different number of nodes stored

The priorities for the implementations were:
* correctness - implement unsafe pointer version and test the basic operations, and additionally verify that the tree height remains logarithmic compared to the number of nodes in the tree
* compare the performance of a pointer-based tree to an arena-based tree (backed by either a vector or a hashmap)

Cleanliness of the code and being production ready were not priorities for this project. For instance, an alternative implementation of an AVL tree can be made without keeping a back reference from a node to its parent. This simplifies ownership and implementation. It would allow for a Box or RefCell implementation with much cleaner internals and less borrowing / ownership trouble. [Here is an example by Francis Murillo](https://github.com/FrancisMurillo/avl_tree_set_rs/blob/master/src/tree.rs). However implementations that do not keep a reference from a node to its parent need to keep a stack "on the way down" the tree.

## Implementations

`avl_unsafe`:

Uses `Option<NonNull<Node<T>>>` as link between nodes. Nodes get allocated with `NonNull::new_unchecked(Box::into_raw(Box::new(Node {` and deallocated (to prevent memory leaks) with `Box::from_raw(node_ptr.as_ptr())` and letting the scope drop the node. The pattern is well described in the [Entirely Too Many Linked Lists](https://rust-unofficial.github.io/too-many-lists/sixth-basics.html) book.

The unsafe version was tested with [miri](https://rust-unofficial.github.io/too-many-lists/fifth-miri.html) for memory leaks, undefined behaviour and other bugs.

`avl_vec`:

Slab arena based implementation in which nodes store `Option<usize>` to an index in a `Vec`. The nodes themselves are stored in the vector. After removing a node, the vector is not shrunk. Instead, the item at the index is marked as free and over time the tree builds a singly linked list of free items. This implementation does not suffer from the [ABA problem](https://en.wikipedia.org/wiki/ABA_problem) because it ensures that all references to a node (and its index) are cleared before the node itself is removed.

`avl_hashmap`:

Another arena implementation which stores the nodes in a `HashMap<usize, Node<T>>`. The usize key is autoincrementing and maintained by the tree. This is simpler than maintaining a vector (and the complexities of a linked list of free slots). However hashing the keys incurs a cost.

## Expectations
 Before starting the project, my expectation was that arena based implementations (both vector and hashmap based, but vector in particular) would be faster than the unsafe pointer based implementation. The theoretical reason to expect pointer implementation to be slower is because each node is allocated independently which means the nodes would be spread randomly in memory. This should result in a high percentage of cache misses and thus bad performance. On the other hand, a vector keeps its elements in a continuous block of memory and should result in good cache locality. This is a reasonable theory but ignores some important details of binary trees that are relevant in practice.

## Findings
I built a very simple "benchmark" binary which prints timings for loop of `inserts`, followed by `contains` and finally `removes`.

For all 3 implementations I verified that 10x increase in number of nodes results ~12x-13x increase in execution time. That is consistent with our expectations given we are doing n operations that take O(logn) each, or in total O(n * logn) time. This is evidence that the algorithmic complexity of the tree operations is correct, and differences in performance are due to underlying implemenentation rather than differences (or bugs) in the algorithm for inserts/removes.


| Implementation | Insert (ms) | Contains (ms) | Remove (ms) | Total (ms) |
|---|---|---|---|---|
| unsafe 10,000 | 3.448 | 0.45 | 1.023 | 4.922 |
| vec 10,000 | 3.595 | 0.881 | 1.968 | 6.445 |
| unsafe 100,000 | 29.178 | 7.574 | 9.115 | 45.868 |
| vec 100,000 | 52.113 | 9.529 | 16.145 | 77.788 |
| unsafe 1,000,000 | 186.726 | 51.697 | 85.405 | 323.828 |
| vec 1,000,000 | 368.139 | 99.994 | 186.619 | 654.753 |

The hashmap implementation is considerably slower than the other two implementations and is out of scope for this analysis. Reverse flamegraph of it reveals that in this microbenchmark, unsurprisingly, most of the time is spent on hashing.

The surprising findings are that the unsafe pointer implementation is 2x faster than the vector implementation when dealing with 1M nodes. Comparing flamegraphs demonstrates that proportionally both implementation spend roughly the same amount of time on the same operations (i.e. rebalancing takes ~40% of time for both). However, the vec implementation is half as fast.

Running both under `valgrind --tool=cachegrind` with 1M nodes paints an interesting picture

| Metric | Unsafe Pointers | Vector Arena | Impact |
|--------|----------------|--------------|---------|
| **I refs** | 2,400,627,178 | 5,449,094,038 | **2.3x more** |
| **D refs** | 823,266,332 | 1,511,678,404 | **1.8x more** |
| **LL refs** | 56,424,343 | 298,633,957 | **5.3x more** |
| **D1 misses** | 56,421,442 | 298,630,985 | **5.3x more** |
| **LLd misses** | 8,054,245 | 208,043,423 | **25.8x more** |
| **LL misses** | 8,056,453 | 208,045,671 | **25.8x more** |
| **D1 miss rate** | 6.9% | 19.8% | |
| **LLd miss rate** | 1.0% | 13.8% | |
| **LL miss rate** | 0.2% | 3.0% | |

We expected that using a vector to store nodes would improve cache locality. Instead, the vector version is much worse both in absolute and relative terms.

The most probable reason why the cache locality benefit never materialised is due to the nature of the binary tree we are implementing. In our test each node is storing a 32 bit value, but also contains 3 usize (64 bit) references (left, right, parent). Each node's memory usage is dominated by the references themselves. Additionally, locality of nodes should not have been a factor we expect in the first place. A slightly different data structure can give us an idea why - priority heap (heap queue). Priority heaps are [complete binary trees](https://www.geeksforgeeks.org/dsa/complete-binary-tree/). That makes them particularly suited to be stored in a vector, where the references to children are not stored anywhere and are instead calculated using a formula - `left_child_index = 2 * parent_index + 1` and `right_child_index = 2 * parent_index + 2`. From this we can make the observation that as the priority heap grows, so does the distance to the two children which makes it less likely to be a cache hit. Our AVL trees exhibit even worse cache locality, and also are affected by rebalance operations to maintain O(logn) height. These rebalance operations must be introducing even more uncertainty.

On the other hand, the vector implementation is doing more work. In the pointer implementation, moving between nodes is done by unsafe raw pointer dereferencing. This is a fast and direct operation. The vector version uses vector indexing. The Rust compiler adds out of bounds checks that run all the time. Additionally, the items in the vector are part of an enum - either a node, or a "free slot" part of the freelist. Our microbenchmark never creates "free slot" gaps in the vector. This works in the vector implementation favour. However the enum matching code needs to run at all times anyway. Both the vector index bounds checks and the enum matching add considerable amount of extra work that is missing from the raw pointer version.

## Conclusion
This project demonstrated that memory layout optimisations don't always translate to performance gains when the access patterns don't align with the optimisation strategy. AVL trees, with their non-sequential access patterns and rebalancing operations, don't benefit from the theoretical cache locality advantages of vector storage. Instead, the overhead of bounds checking and enum pattern matching in the vector implementation, combined with the lack of spatial locality in tree traversal, results in significantly worse performance compared to the direct pointer approach.

The key learning is that whilst memory locality is important for performance, it must be considered in the context of the actual data access patterns of the algorithm being implemented.
