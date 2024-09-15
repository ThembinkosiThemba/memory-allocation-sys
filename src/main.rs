use std::{
    alloc::{alloc, dealloc, Layout},
    collections::HashMap,
    marker::PhantomData,
    ptr::NonNull,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

// This enum represents different subscription levels for clients, which determine their memory allocation limit
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum SubscriptionTier {
    Basic,
    Premium,
    Enterprise,
}

// Represents a block of allocated memory. It contains a pointer to the memory (`ptr`), the size of the block, and a `PhantomData` marker.
#[derive(Clone)]
struct MemoryBlock {
    ptr: NonNull<u8>,
    size: usize,
    _marker: PhantomData<u8>, // indication of owning the data
}

unsafe impl Send for MemoryBlock {}
unsafe impl Sync for MemoryBlock {}

impl MemoryBlock {
    fn new(size: usize) -> Option<Self> {
        let layout = Layout::from_size_align(size, 8).ok()?;
        let ptr = unsafe { NonNull::new(alloc(layout))? };
        Some(MemoryBlock {
            ptr,
            size,
            _marker: PhantomData,
        })
    }
}

impl Drop for MemoryBlock {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.ptr.as_ptr(),
                Layout::from_size_align_unchecked(self.size, 8),
            );
        }
    }
}

// Keeps track of a client's allocation details, including their subscription tier, allocated memory blocks, used memory, and when it was last updated
struct ClientAllocation {
    tier: SubscriptionTier,
    blocks: Vec<MemoryBlock>,
    used: usize,
    last_updated: Instant,
}

// The main allocator struct. It manages the overall memory pool, keeps track of client allocations, enforces tier limits, and handles memory reclamation.
struct TieredAllocator {
    total_memory: usize,
    usable_memory: usize,
    allocated_memory: usize,
    clients: HashMap<String, ClientAllocation>,
    tier_limits: HashMap<SubscriptionTier, usize>,
    last_reclaim: Instant,
}

impl TieredAllocator {
    fn new(total_memory: usize) -> Self {
        let usable_memory = (total_memory as f64 * 0.8) as usize;
        let mut tier_limits = HashMap::new();
        tier_limits.insert(SubscriptionTier::Basic, usable_memory / 10);
        tier_limits.insert(SubscriptionTier::Premium, usable_memory / 5);
        tier_limits.insert(SubscriptionTier::Enterprise, usable_memory / 2);

        TieredAllocator {
            total_memory,
            usable_memory,
            allocated_memory: 0,
            clients: HashMap::new(),
            tier_limits,
            last_reclaim: Instant::now(),
        }
    }

    // allocates memory for a client, respecting tier limits
    fn allocate(&mut self, client_id: &str, size: usize) -> Result<MemoryBlock, String> {
        let client = self.clients.get_mut(client_id).ok_or("Client not found")?;

        let client_total_allocated = client.blocks.iter().map(|b| b.size).sum::<usize>();
        if client_total_allocated + size > self.tier_limits[&client.tier] {
            return Err("Exceeds tier limit".to_string());
        }

        if self.allocated_memory + size > self.usable_memory {
            return Err("Out of memory".to_string());
        }

        let block = MemoryBlock::new(size).ok_or("Memory allocation failed")?;

        client.blocks.push(block.clone());
        self.allocated_memory += size;

        Ok(block.clone())
    }

    // frees allocated memory for a client
    fn deallocate(&mut self, client_id: &str, ptr: NonNull<u8>) -> Result<(), String> {
        let client = self.clients.get_mut(client_id).ok_or("Client not found")?;

        let block_index = client
            .blocks
            .iter()
            .position(|b| b.ptr == ptr)
            .ok_or("Memory block not found")?;
        let block = client.blocks.remove(block_index);

        self.allocated_memory -= block.size;
        Ok(())
    }

    fn update_usage(&mut self, client_id: &str, used: usize) -> Result<(), String> {
        let client = self.clients.get_mut(client_id).ok_or("Client not found")?;
        client.used = used;
        client.last_updated = Instant::now();
        Ok(())
    }

    fn add_client(&mut self, client_id: String, tier: SubscriptionTier) {
        self.clients.insert(
            client_id,
            ClientAllocation {
                tier,
                blocks: Vec::new(),
                used: 0,
                last_updated: Instant::now(),
            },
        );
    }

    fn get_client_usage(&self, client_id: &str) -> Result<(usize, usize), String> {
        let client = self.clients.get(client_id).ok_or("Client not found")?;
        let allocated = client.blocks.iter().map(|b| b.size).sum();
        Ok((allocated, client.used))
    }

    fn get_total_usage(&self) -> (usize, usize) {
        let used = self.clients.values().map(|c| c.used).sum();
        (self.allocated_memory, used)
    }

    // Reclaims unused memory from clients.
    fn reclaim_unused_memory(&mut self) {
        for client in self.clients.values_mut() {
            client.blocks.retain(|block| client.used >= block.size);
            client.used = client
                .used
                .saturating_sub(client.blocks.iter().map(|b| b.size).sum::<usize>());
        }
        self.allocated_memory = self
            .clients
            .values()
            .flat_map(|c| &c.blocks)
            .map(|b| b.size)
            .sum();
        self.last_reclaim = Instant::now();
    }
}

// A wrapper around TieredAllocator that provides a thread-safe interface for memory allocation and management.
struct Memory {
    allocator: Arc<Mutex<TieredAllocator>>,
}

impl Memory {
    fn new(total_memory: usize) -> Arc<Self> {
        let mem = Arc::new(Memory {
            allocator: Arc::new(Mutex::new(TieredAllocator::new(total_memory))),
        });

        Self::start_memory_reclaimer(Arc::clone(&mem));
        Self::start_usage_updater(Arc::clone(&mem));

        mem
    }

    // Starts a background thread for periodic memory reclamation.
    fn start_memory_reclaimer(db: Arc<Self>) {
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(60)); // Run every minute
                let mut allocator = db.allocator.lock().unwrap();
                if allocator.last_reclaim.elapsed() > Duration::from_secs(300) {
                    // 5 minutes
                    allocator.reclaim_unused_memory();
                }
            }
        });
    }

    // Starts a background thread to update client usage statistics.
    fn start_usage_updater(db: Arc<Self>) {
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(10)); // Run every 10 seconds
                                                        // This is where you'd implement logic to update usage for all clients
                let mut allocator = db.allocator.lock().unwrap();
                for (_, allocation) in allocator.clients.iter_mut() {
                    // This is a placeholder. In a real implementation, you'd have a way to measure actual usage.
                    let actual_usage = allocation.blocks.iter().map(|b| b.size).sum::<usize>() / 2; // Example: assume half of allocated is used
                    allocation.used = actual_usage;
                    allocation.last_updated = Instant::now();
                }
            }
        });
    }

    fn allocate_for_client(&self, client_id: &str, size: usize) -> Result<MemoryBlock, String> {
        let mut allocator = self.allocator.lock().unwrap();
        allocator.allocate(client_id, size)
    }

    fn deallocate_for_client(&self, client_id: &str, block: MemoryBlock) -> Result<(), String> {
        let mut allocator = self.allocator.lock().unwrap();
        allocator.deallocate(client_id, block.ptr)
    }

    fn update_client_usage(&self, client_id: &str, used: usize) -> Result<(), String> {
        let mut allocator = self.allocator.lock().unwrap();
        allocator.update_usage(client_id, used)
    }

    fn get_resource_usage(&self) -> (usize, usize) {
        let allocator = self.allocator.lock().unwrap();
        allocator.get_total_usage()
    }

    fn get_client_resource_usage(&self, client_id: &str) -> Result<(usize, usize), String> {
        let allocator = self.allocator.lock().unwrap();
        allocator.get_client_usage(client_id)
    }

    fn reclaim_unused_memory(&self) {
        let mut allocator = self.allocator.lock().unwrap();
        allocator.reclaim_unused_memory();
    }
}
fn main() {
    let mem = Memory::new(1_000_000_000);

    // example usage
    {
        let mut allocator = mem.allocator.lock().unwrap();
        allocator.add_client("client_1".to_string(), SubscriptionTier::Premium)
    }

    let block = mem.allocate_for_client("client_1", 50_000_000).unwrap();

    thread::sleep(Duration::from_secs(120));

    let (allocated, used) = mem.get_client_resource_usage("client_1").unwrap();
    println!("Client usage: {}/{} bytes", used, allocated);

    let (total_allocated, total_used) = mem.get_resource_usage();
    println!("Total usage: {}/{} bytes", total_used, total_allocated);

    mem.deallocate_for_client("client_1", block).unwrap();
}
