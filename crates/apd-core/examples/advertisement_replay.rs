//! Replay identical advertisements to measure the steady-state hot path.
//! Run with: cargo run -p apd-core --release --locked --example advertisement_replay

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use apd_core::{protocol::AirPodsPacket, Manager};

struct CountingAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        System.realloc(ptr, layout, size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() {
    let mut raw = [0u8; 27];
    raw[0] = 7;
    raw[1] = 25;
    raw[3] = 0x0e;
    raw[4] = 0x20;
    raw[5] = 0x2a;
    raw[6] = 0x88;
    raw[7] = 6;
    let packet = AirPodsPacket::parse(&raw).expect("valid replay packet");
    let manager = Manager::new();
    manager.on_advertisement(packet.clone(), 0xaa, -40);

    const ITERATIONS: usize = 500_000;
    let started = Instant::now();
    ALLOCATIONS.store(0, Ordering::Relaxed);
    for _ in 0..ITERATIONS {
        black_box(manager.on_advertisement(black_box(packet.clone()), 0xaa, -40));
    }
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let elapsed = started.elapsed();
    println!("{ITERATIONS} duplicate advertisements: {allocations} allocations, {elapsed:?}");
}
