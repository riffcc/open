use std::collections::{HashMap, VecDeque};
use std::f64;
use std::time::Instant;

struct GlobalHexNetwork {
    node_count: usize,
    layers: usize,
    latencies: HashMap<&'static str, f64>,
}

impl GlobalHexNetwork {
    fn new(target_nodes: usize) -> Self {
        let layers = Self::calculate_layers(target_nodes);
        let node_count = Self::calculate_total_nodes(layers);
        let mut latencies = HashMap::new();
        latencies.insert("local", 5.0); // Placeholder for mean latency
        latencies.insert("regional", 25.0);
        latencies.insert("global", 100.0);
        
        Self {
            node_count,
            layers,
            latencies,
        }
    }

    fn calculate_layers(target: usize) -> usize {
        let mut nodes = 1;
        let mut layer = 0;
        while nodes < target {
            layer += 1;
            nodes += 6 * layer;
        }
        layer
    }

    fn calculate_total_nodes(layers: usize) -> usize {
        (1..=layers).map(|layer| 6 * layer).sum::<usize>() + 1
    }

    fn propagate_signal(&self, start_node: usize) -> (f64, usize) {
        let mut visited = vec![false; self.node_count];
        let mut queue = VecDeque::new();
        queue.push_back((start_node, 0.0));
        
        let mut max_time: f64 = 0.0;
        let mut nodes_reached = 0;
        
        while let Some((node, time)) = queue.pop_front() {
            if visited[node] {
                continue;
            }
            visited[node] = true;
            nodes_reached += 1;
            max_time = max_time.max(time);

            for &(neighbor, latency) in &self.get_neighbors(node) {
                if !visited[neighbor] {
                    queue.push_back((neighbor, time + latency));
                }
            }
        }
        (max_time, nodes_reached)
    }

    fn get_neighbors(&self, node: usize) -> Vec<(usize, f64)> {
        let local_neighbors = vec![node + 1, node.saturating_sub(1)];
        let regional_neighbors = vec![node + 1000, node.saturating_sub(1000)];
        let mut neighbors = Vec::new();

        for n in local_neighbors {
            if n < self.node_count {
                neighbors.push((n, *self.latencies.get("local").unwrap()));
            }
        }
        
        for n in regional_neighbors {
            if n < self.node_count {
                neighbors.push((n, *self.latencies.get("regional").unwrap()));
            }
        }
        neighbors
    }
    
    fn simulate_attack(&self, failure_rate: f64) -> Vec<(f64, usize)> {
        let total_failures = (self.node_count as f64 * failure_rate).round() as usize;
        let failed_nodes: Vec<_> = (0..self.node_count).collect::<Vec<_>>()
            .into_iter()
            .take(total_failures)
            .collect();
        
        let mut results = Vec::new();
        for _ in 0..3 {
            let start_node = (0..self.node_count)
                .find(|n| !failed_nodes.contains(n))
                .unwrap();
            results.push(self.propagate_signal_under_attack(start_node, &failed_nodes));
        }
        results
    }

    fn propagate_signal_under_attack(&self, start_node: usize, failed_nodes: &[usize]) -> (f64, usize) {
        let mut visited = vec![false; self.node_count];
        let mut queue = VecDeque::new();
        queue.push_back((start_node, 0.0));
        
        let mut max_time: f64 = 0.0;
        let mut nodes_reached = 0;
        
        while let Some((node, time)) = queue.pop_front() {
            if visited[node] || failed_nodes.contains(&node) {
                continue;
            }
            visited[node] = true;
            nodes_reached += 1;
            max_time = max_time.max(time);

            for &(neighbor, latency) in &self.get_neighbors(node) {
                if !visited[neighbor] && !failed_nodes.contains(&neighbor) {
                    queue.push_back((neighbor, time + latency));
                }
            }
        }
        (max_time, nodes_reached)
    }
}

fn main() {
    let network = GlobalHexNetwork::new(100_000_000_000); // Example target node count
    
    println!("Running network propagation...");
    let start_time = Instant::now();
    let (max_time, nodes_reached) = network.propagate_signal(0);
    let duration = start_time.elapsed();

    println!("Max propagation time: {:.2} ms", max_time);
    println!("Nodes reached: {}", nodes_reached);
    println!("Propagation took: {:.2?}", duration);

    println!("\nSimulating attack scenarios...");
    for &rate in &[0.3, 0.5, 0.7] {
        println!("\nSimulating {}% node failure", rate * 100.0);
        let results = network.simulate_attack(rate);
        for (i, (time, reached)) in results.iter().enumerate() {
            println!("Round {}: Reached {} nodes, Max time: {:.2} ms", i + 1, reached, time);
        }
    }
}
