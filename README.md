# Tiered Memory Allocator
[Note, This will get some improvements]

## Overview

The Tiered Memory Allocator is a Rust-based memory management system designed to allocate and manage memory for multiple clients with different subscription tiers. It provides efficient memory allocation, deallocation, and automatic reclamation of unused memory.

The project demonstrates low-level memory management in Rust, using `alloc` and `dealloc` from the standard library's `alloc` module. It also showcases how to build a higher-level memory management system on top of these low-level operations.

## Features

- Tiered memory allocation based on client subscription levels (Basic, Premium, Enterprise)
- Thread-safe memory allocation and deallocation
- Automatic memory reclamation for unused blocks
- Usage tracking for individual clients and overall system
- Background threads for periodic memory reclamation and usage updates

## Unsafe Rust Usage

- This project uses unsafe rust in several places where the code needs to interact directly with memory. You will see keywords like `alloc()` which is used to allocate raw memory, `dealloc()` which is used to free the allocated memory.
- `unsafe impl Send for MemoryBlock {}` and `unsafe impl Sync for MemoryBlock {}` are used to mark `MemoryBlock` as safe to send between threads and safe to share between threads, respectively.

Unsafe Rust is necessary here becasue these operations involve raw pointer manilulation and memory management, which the Rust compiler can't guarantee as safe.

## PhantomData

`PhantomData` is used in the `MemoryBlock` struct:

```rust
struct MemoryBlock {
    ptr: NonNull<u8>,
    size: usize,
    _marker: PhantomData<u8>,
}
```

`PhantomData<u8>` is ued to indicate that `MemoryBlock` logically owns a `u8` value, even though it doesn't actually contain one. This is important for correct `Drop` behavior and to prevent the compiler from thinking the type is invariant over `u8`.

The `ptr` field is a pointer to the beginning of the allocated memory block. It's of type `NonNull<u8>`, which is a wrapper around a raw pointer that is guaranteed to be non-null.

## Smart Pointers

- `NonNull<u8>` is used for the raw pointer in `MemoryBlock`, `NonNull` is a wrapper around raw pointers that's guaranteed to be a non-null.
- `Arc<Mutex<TieredAllocator>>` is used in the `Memory` struct to allow safe sharing of the allocator between threads.

## Requirements

- Rust (latest stable version recommended)

## Usage

1. Include the Tiered Allocator in your project:

   ```rust
   use tiered_allocator::Memory;
   ```

2. Create a new Memory instance:

   ```rust
   let mem = Memory::new(1_000_000_000); // 1GB total memory
   ```

3. Add clients with their respective tiers:

   ```rust
   mem.add_client("client_1", SubscriptionTier::Premium);
   ```

4. Allocate memory for clients:

   ```rust
   let block = mem.allocate_for_client("client_1", 50_000_000).unwrap();
   ```

5. Deallocate memory when no longer needed:

   ```rust
   mem.deallocate_for_client("client_1", block).unwrap();
   ```

6. Get resource usage information:

   ```rust
   let (allocated, used) = mem.get_client_resource_usage("client_1").unwrap();
   let (total_allocated, total_used) = mem.get_resource_usage();
   ```

## Important Notes

- The allocator uses unsafe Rust for low-level memory management. Ensure that all memory blocks are properly deallocated to prevent memory leaks.
- The system automatically reclaims unused memory periodically, but it's good practice to deallocate memory explicitly when it's no longer needed.
- The allocator is thread-safe, but be cautious when sharing memory blocks between threads.

## Contributing

Contributions to the Tiered Memory Allocator are welcome! Please feel free to submit pull requests or open issues to suggest improvements or report bugs.
