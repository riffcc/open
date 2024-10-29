use std::collections::{HashMap, HashSet, BinaryHeap};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;
use std::env;
use std::io;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::layout::{Layout, Constraint, Direction};
use ratatui::style::{Style, Color};
use crossterm::event::{self, Event, KeyCode};
use rand::Rng;

#[derive(Copy, Clone)]
struct Node {
    index: usize,
    time: f64,
    depth: usize,
    hash: u64,
}

impl Node {
    fn new(index: usize, time: f64, depth: usize) -> Self {
        let hash = Self::calculate_hash(index, depth);
        Self { index, time, depth, hash }
    }

    fn calculate_hash(index: usize, depth: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        index.hash(&mut hasher);
        depth.hash(&mut hasher);
        hasher.finish()
    }

    fn get_fractal_neighbors(&self, network: &GlobalHexNetwork) -> Vec<(Node, f64)> {
        let mut neighbors = Vec::new();
        
        // Same level neighbors
        for i in 1..=6 {
            let neighbor_index = (self.hash + i as u64) % network.node_count as u64;
            neighbors.push((
                Node::new(neighbor_index as usize, self.time, self.depth),
                network.get_latency("local")
            ));
        }

        // Upper level (parent) connection
        if self.depth > 0 {
            let parent_index = self.index / 7;
            neighbors.push((
                Node::new(parent_index, self.time, self.depth - 1),
                network.get_latency("regional")
            ));
        }

        // Lower level (children) connections
        if self.depth < network.max_depth {
            for i in 0..6 {
                let child_index = self.index * 7 + i + 1;
                if child_index < network.node_count {
                    neighbors.push((
                        Node::new(child_index, self.time, self.depth + 1),
                        network.get_latency("regional")
                    ));
                }
            }
        }

        neighbors
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.partial_cmp(&self.time).unwrap()
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct GlobalHexNetwork {
    node_count: usize,
    max_depth: usize,
    latencies: HashMap<&'static str, f64>,
    fractal_mode: bool,
}

impl GlobalHexNetwork {
    fn new(target_nodes: usize, fractal_mode: bool) -> Self {
        let max_depth = if fractal_mode {
            (target_nodes as f64).log(7.0).ceil() as usize
        } else {
            1
        };
        
        let mut latencies = HashMap::new();
        latencies.insert("local", 5.0);
        latencies.insert("regional", 25.0);
        latencies.insert("global", 100.0);
        
        Self {
            node_count: target_nodes,
            max_depth,
            latencies,
            fractal_mode,
        }
    }

    fn get_latency(&self, connection_type: &str) -> f64 {
        let base_latency = match connection_type {
            "local" => 5.0,
            "regional" => 25.0,
            "global" => 100.0,
            _ => 5.0,
        };

        // Add realistic network effects:
        // 1. Random jitter (1-5% variation)
        let jitter = rand::thread_rng().gen_range(0.01..=0.05);
        
        // 2. Distance-based delay
        let distance_factor = 1.0 + (rand::thread_rng().gen_range(0.0..=0.1));
        
        // 3. Network congestion simulation
        let congestion = 1.0 + (self.node_count as f64 / 1_000_000.0);

        base_latency * (1.0 + jitter) * distance_factor * congestion
    }

    fn propagate_signal(&self, start_node: usize) -> (f64, usize) {
        if self.fractal_mode {
            self.propagate_fractal(start_node)
        } else {
            self.propagate_flat(start_node)
        }
    }

    fn propagate_flat(&self, start_node: usize) -> (f64, usize) {
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();
        heap.push(Node::new(start_node, 0.0, 0));
        
        let mut max_time: f64 = 0.0;
        let mut nodes_reached = 0;
        
        while let Some(node) = heap.pop() {
            if visited.contains(&node.hash) {
                continue;
            }
            
            // Add processing delay
            std::thread::sleep(std::time::Duration::from_micros(1));
            
            visited.insert(node.hash);
            nodes_reached += 1;
            max_time = max_time.max(node.time);

            // Get neighbors using consistent hashing
            let neighbor_hashes = (1..=6).map(|i| {
                let mut hasher = DefaultHasher::new();
                (node.hash + i as u64).hash(&mut hasher);
                hasher.finish() % self.node_count as u64
            });

            for neighbor_hash in neighbor_hashes {
                if !visited.contains(&neighbor_hash) {
                    heap.push(Node::new(
                        neighbor_hash as usize,
                        node.time + self.get_latency("local"),
                        0
                    ));
                }
            }
        }
        (max_time, nodes_reached)
    }

    fn propagate_fractal(&self, start_node: usize) -> (f64, usize) {
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();
        heap.push(Node::new(start_node, 0.0, 0));
        
        let mut max_time: f64 = 0.0;
        let mut nodes_reached = 0;
        
        while let Some(node) = heap.pop() {
            if visited.contains(&node.hash) {
                continue;
            }
            visited.insert(node.hash);
            nodes_reached += 1;
            
            // Debug print
            println!("Nodes reached: {}, Total nodes: {}, Percentage: {}%", 
                nodes_reached, 
                self.node_count,
                (nodes_reached as f64 / self.node_count as f64 * 100.0));
            
            max_time = max_time.max(node.time);

            for (neighbor, latency) in node.get_fractal_neighbors(self) {
                if !visited.contains(&neighbor.hash) {
                    heap.push(Node::new(
                        neighbor.index,
                        node.time + latency,
                        neighbor.depth
                    ));
                }
            }
        }
        (max_time, nodes_reached)
    }
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    let node_count = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10_000);
    let fractal_mode = args.contains(&"--fractal".to_string());

    let network = GlobalHexNetwork::new(node_count, fractal_mode);
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let (max_time, nodes_reached) = network.propagate_signal(0);
    let progress = (nodes_reached as f64 / network.node_count as f64 * 100.0)
        .min(100.0)
        .max(0.0);

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                    ]
                    .as_ref(),
                )
                .split(f.area());

            let mode_str = if network.fractal_mode { "Fractal" } else { "Flat" };
            let title = format!("GlobalHexNetwork Simulation (Mode: {})", mode_str);
            let block = Block::default().title(title).borders(Borders::ALL);
            f.render_widget(block, chunks[0]);

            let gauge = Gauge::default()
                .block(Block::default().title("Propagation Progress").borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(progress.round() as u16);
            f.render_widget(gauge, chunks[1]);

            let stats = Paragraph::new(format!(
                "Nodes Reached: {}\nMax Time: {:.2} ms\nMax Depth: {}\nTotal Nodes: {}",
                nodes_reached,
                max_time,
                network.max_depth,
                network.node_count
            ))
            .block(Block::default().title("Propagation Stats").borders(Borders::ALL));
            f.render_widget(stats, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    terminal.clear()?;
    println!("Final Propagation Stats:");
    println!("Mode: {}", if network.fractal_mode { "Fractal" } else { "Flat" });
    println!("Nodes Reached: {}", nodes_reached);
    println!("Max Time: {:.2} ms", max_time);
    println!("Max Depth: {}", network.max_depth);
    println!("Total Nodes: {}", network.node_count);

    Ok(())
}
