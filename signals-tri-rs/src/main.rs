use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::Mutex;
use std::collections::HashSet;
use once_cell::sync::Lazy;

// Simulation parameters
const TARGET_NODES: u64 = 10_000; // Higher target to observe exponential growth
const HEX_HOP_TIME_MS: u64 = 50; // Time per hop in milliseconds
const LOG_INTERVAL_PERCENT: u64 = 1; // Log progress every 1% of target nodes

// Global atomic counters
static TOTAL_NODES: AtomicU64 = AtomicU64::new(1); // Start with the initial node
static HOPS: AtomicU64 = AtomicU64::new(0); // Count of hops taken

// Bitset for visited nodes
static VISITED_NODES: Lazy<Mutex<HashSet<u64>>> = Lazy::new(|| Mutex::new(HashSet::new()));

// Structure to represent a node's coordinates in axial (q, r) format
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
struct Node(i64, i64);

// Define axial direction vectors for a hexagonal grid
const HEX_DIRECTIONS: [(i64, i64); 6] = [
    (1, 0),   // Right
    (0, 1),   // Top-right
    (-1, 1),  // Top-left
    (-1, 0),  // Left
    (0, -1),  // Bottom-left
    (1, -1),  // Bottom-right
];

// Function to calculate neighbors in hexagonal paths using axial coordinates
fn get_hex_neighbors(node: Node) -> Vec<Node> {
    HEX_DIRECTIONS
        .iter()
        .map(|&(dq, dr)| Node(node.0 + dq, node.1 + dr))
        .collect()
}

// Simple hashing function for coordinates (q, r) to a unique index
fn node_to_index(node: Node) -> u64 {
    ((node.0 as u64) << 32) | (node.1 as u64)
}

// Stateless function to simulate firing packets to neighbors
fn fire_packet(node: Node, target_nodes: u64) {
    let node_index = node_to_index(node);

    // Check if node has been visited, if so return immediately
    {
        let mut visited = VISITED_NODES.lock().unwrap();
        if !visited.insert(node_index) {
            return;
        }
    }

    // Simulate latency for this specific node
    thread::sleep(Duration::from_millis(HEX_HOP_TIME_MS));

    // Fire packets to each of the hex neighbors
    for neighbor in get_hex_neighbors(node) {
        let node_count = TOTAL_NODES.fetch_add(1, Ordering::SeqCst) + 1;
        let hop_count = HOPS.fetch_add(1, Ordering::SeqCst) + 1;

        // Display a progress bar every LOG_INTERVAL_PERCENT% of TARGET_NODES
        let progress_threshold = TARGET_NODES * LOG_INTERVAL_PERCENT / 100;
        if node_count % progress_threshold == 0 {
            let percentage = (node_count as f64 / target_nodes as f64) * 100.0;
            println!(
                "[{:.0}%] - Nodes: {}, Hops: {}, Elapsed Time: {:.2} sec",
                percentage,
                node_count,
                hop_count,
                (hop_count * HEX_HOP_TIME_MS) as f64 / 1000.0
            );
        }

        // Spawn a new thread for each neighbor, allowing parallel processing
        if node_count < target_nodes {
            thread::spawn(move || fire_packet(neighbor, target_nodes));
        }
    }
}

fn main() {
    // Start timing
    let start = Instant::now();

    // Start firing packets from the origin node
    fire_packet(Node(0, 0), TARGET_NODES);

    // Final results
    let total_hops = HOPS.load(Ordering::SeqCst);
    let elapsed = start.elapsed();
    println!("\nTotal hops taken: {}", total_hops);
    println!("Total time elapsed: {:.2} seconds", elapsed.as_secs_f64());
}
