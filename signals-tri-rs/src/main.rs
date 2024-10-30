use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Simulation parameters
const TARGET_NODES: u64 = 10_000_000; // Lower target for demonstration, adjust as needed
const CONNECTIONS_PER_NODE: u64 = 3; // Each node connects to 3 others initially
const HEX_CONNECTIONS: u64 = 6; // Each triangle node expands to 6 neighbors in hex routing
const HEX_HOP_TIME_MS: u64 = 50; // Time per hop in milliseconds

// Global atomic counters
static TOTAL_NODES: AtomicU64 = AtomicU64::new(1); // Start with the initial node
static HOPS: AtomicU64 = AtomicU64::new(0); // Count of hops taken

// Function to simulate packet traversal with latency
fn traverse_packet(current_hop: u64, target_nodes: u64, total_nodes: Arc<AtomicU64>, hops: Arc<AtomicU64>) {
    let mut local_nodes = 1; // Start with the initial node at this hop
    let mut current_hop_count = current_hop;

    while local_nodes < target_nodes {
        thread::sleep(Duration::from_millis(HEX_HOP_TIME_MS)); // Simulate latency per hop
        local_nodes *= CONNECTIONS_PER_NODE * HEX_CONNECTIONS; // Growth factor

        // Update total counters atomically
        total_nodes.fetch_add(local_nodes, Ordering::SeqCst);
        hops.fetch_add(1, Ordering::SeqCst);

        current_hop_count += 1;

        // Print current progress
        let total = total_nodes.load(Ordering::SeqCst);
        let elapsed_time_seconds = (current_hop_count * HEX_HOP_TIME_MS) as f64 / 1000.0;
        println!(
            "Hop {}: Reached {} nodes, Time elapsed {:.2} seconds",
            current_hop_count, total, elapsed_time_seconds
        );

        if total >= target_nodes {
            break;
        }
    }
}

fn main() {
    let target_nodes = TARGET_NODES;
    let total_nodes = Arc::new(AtomicU64::new(1));
    let hops = Arc::new(AtomicU64::new(0));

    // Start packet traversal simulation in a thread
    let total_nodes_clone = Arc::clone(&total_nodes);
    let hops_clone = Arc::clone(&hops);

    let simulation_handle = thread::spawn(move || {
        traverse_packet(1, target_nodes, total_nodes_clone, hops_clone);
    });

    // Wait for the simulation to complete
    simulation_handle.join().unwrap();

    // Final results
    let total_hops = hops.load(Ordering::SeqCst);
    let total_time_seconds = (total_hops * HEX_HOP_TIME_MS) as f64 / 1000.0;
    println!("\nTotal hops taken: {}", total_hops);
    println!("Total time in seconds: {:.2}", total_time_seconds);
}
