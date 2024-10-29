use std::collections::{HashMap, HashSet, BinaryHeap};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;
use std::env;
use std::io;
use std::time::{Duration, Instant};
use rand::Rng;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::{Block, Borders, canvas::Canvas, Paragraph},
    layout::{Layout, Constraint, Direction, Rect},
};
use crossterm::event::{self, Event, KeyCode};
use std::sync::mpsc::channel;
use std::thread;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug)]
struct WaveFront {
    ring: usize,
    nodes: usize,
    time: f64,
    start_time: Instant,
}

struct HexPoint {
    x: f64,
    y: f64,
    ring: usize,
}

impl HexPoint {
    fn new(ring: usize, segment: usize) -> Self {
        let angle = (segment as f64) * std::f64::consts::PI / 3.0;
        let radius = ring as f64;
        Self {
            x: radius * angle.cos() * 2.0, // Multiply by 2.0 to space out the hexes
            y: radius * angle.sin() * 2.0,
            ring,
        }
    }
}

struct PropagationVisualizer {
    current_wave: usize,
    start_time: Instant,
    hex_points: Vec<HexPoint>,
    current_time: f64,
    nodes_reached: usize,
    last_latency: f64,
}

impl PropagationVisualizer {
    fn new(initial_rings: usize) -> Self {
        let capacity = initial_rings * 6 + 1; // Preallocate exact space needed
        let mut hex_points = Vec::with_capacity(capacity);
        
        // Generate points lazily
        hex_points.push(HexPoint { x: 0.0, y: 0.0, ring: 0 });
        
        Self {
            current_wave: 0,
            start_time: Instant::now(),
            hex_points,
            current_time: 0.0,
            nodes_reached: 0,
            last_latency: 0.0,
        }
    }

    fn ensure_ring_exists(&mut self, ring: usize) {
        while self.hex_points.len() < ring * 6 + 1 {
            let current_ring = self.hex_points.len() / 6;
            for segment in 0..6 {
                self.hex_points.push(HexPoint::new(current_ring, segment));
            }
        }
    }

    fn calculate_zoom(&self) -> f64 {
        let max_radius = self.current_wave as f64;
        20.0 / max_radius.max(1.0) // Automatically zoom out as waves expand
    }

    fn draw(&self, f: &mut ratatui::Frame, area: Rect) {
        let zoom = self.calculate_zoom();
        
        let stats = format!(
            "{:─^50}\n\
             │ Network Time: {:>10.2}ms {:>21} │\n\
             │ Real Time:    {:>10.2}s  {:>21} │\n\
             │ Nodes:        {:>10}    {:>21} │\n\
             │ Rings:        {:>10}    {:>21} │\n\
             │ Last Latency: {:>10.2}ms {:>21} │\n\
             │ Zoom:         {:>10.2}x  {:>21} │\n\
             {:─^50}",
            "─ STATS ─",
            self.current_time,
            "",
            self.start_time.elapsed().as_secs_f64(),
            "",
            self.nodes_reached,
            "",
            self.current_wave,
            "",
            self.last_latency,
            "",
            zoom,
            "",
            "─"
        );

        let canvas = Canvas::default()
            .block(Block::default().borders(Borders::ALL))
            .paint(|ctx| {
                // Draw all points up to current wave
                for point in &self.hex_points {
                    let screen_x = point.x * zoom;
                    let screen_y = point.y * zoom;
                    
                    if screen_x.abs() > 20.0 || screen_y.abs() > 20.0 {
                        continue;
                    }

                    // Use different symbols based on zoom level and ring
                    let symbol = if zoom > 1.0 {
                        if point.ring == self.current_wave { "⬢" } else { "⬡" }
                    } else {
                        if point.ring == self.current_wave { "●" } else { "·" }
                    };
                    
                    ctx.print(screen_x, screen_y, symbol);
                }
            })
            .x_bounds([-20.0, 20.0])
            .y_bounds([-20.0, 20.0]);

        // Render stats in top-right corner with fixed width
        let stats_area = Rect::new(
            area.right() - 52,  // 50 chars wide + 2 for border
            area.top(),
            52,
            8  // Height of stats box
        );
        
        let stats_paragraph = Paragraph::new(stats)
            .block(Block::default());
        
        f.render_widget(canvas, area);
        f.render_widget(stats_paragraph, stats_area);
    }

    fn update_state(&mut self, state: VisualizationState) {
        self.current_wave = state.current_wave;
        self.nodes_reached = state.nodes_reached;
        self.current_time = state.max_time;
        self.last_latency = state.last_latency;
        self.ensure_ring_exists(state.current_wave);
    }
}

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
        let current_depth = network.calculate_depth(self.index);
        let cluster_size = 7_usize.pow(current_depth as u32);
        
        // Local cluster connections (always available)
        let cluster_start = (self.index / cluster_size) * cluster_size;
        for i in 0..6 {
            let neighbor_index = cluster_start + ((self.index - cluster_start + i + 1) % cluster_size);
            if neighbor_index < network.node_count {
                neighbors.push((
                    Node::new(neighbor_index, self.time, current_depth),
                    network.get_latency("local")
                ));
            }
        }

        // Parent connection (if not root)
        if current_depth > 0 {
            let parent_index = self.index / 7;
            if parent_index < network.node_count {
                neighbors.push((
                    Node::new(parent_index, self.time, current_depth - 1),
                    network.get_latency("regional")
                ));
            }
        }

        // Child connections (if space remains)
        let child_base = self.index * 7 + 1;
        for i in 0..6 {
            let child_index = child_base + i;
            if child_index < network.node_count {
                neighbors.push((
                    Node::new(child_index, self.time, current_depth + 1),
                    network.get_latency("global")
                ));
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
        let (node_count, max_depth) = if fractal_mode {
            // Calculate the largest complete fractal that fits
            let complete_depth = (target_nodes as f64).log(7.0).floor() as usize;
            let complete_size = (0..=complete_depth).map(|d| 7_usize.pow(d as u32)).sum();
            
            // If we have remaining nodes, add a partial next level
            let remaining = target_nodes.saturating_sub(complete_size);
            let partial_depth = if remaining > 0 { complete_depth + 1 } else { complete_depth };
            
            // Use all requested nodes up to the next fractal boundary
            (target_nodes.min((0..=partial_depth).map(|d| 7_usize.pow(d as u32)).sum()), 
             partial_depth)
        } else {
            (target_nodes, 1)
        };
        
        // Increased latencies to better show hierarchical effects
        let mut latencies = HashMap::new();
        latencies.insert("local", 0.1);      // 100μs for same-cluster
        latencies.insert("regional", 1.0);    // 1ms for parent/child
        latencies.insert("global", 5.0);      // 5ms for cross-cluster

        Self {
            node_count,
            max_depth,
            latencies,
            fractal_mode,
        }
    }

    fn propagate_signal(&self, start_node: usize) -> (f64, f64, f64, usize) {
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();
        heap.push(Node::new(start_node, 0.0, 0));
        
        let pb = ProgressBar::new(self.node_count as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"));
        
        let mut max_time: f64 = 0.0;
        let mut min_time: f64 = f64::MAX;
        let mut total_time: f64 = 0.0;
        let mut nodes_reached = 0;
        
        while let Some(node) = heap.pop() {
            if visited.contains(&node.hash) {
                continue;
            }
            visited.insert(node.hash);
            nodes_reached += 1;
            
            min_time = min_time.min(node.time);
            max_time = max_time.max(node.time);
            total_time += node.time;
            
            pb.set_position(nodes_reached as u64);
            if nodes_reached % 10000 == 0 {
                pb.set_message(format!("Time: {:.2}ms", max_time));
            }

            for (neighbor_index, connection_type) in self.get_neighbors(node.index) {
                let neighbor_node = Node::new(
                    neighbor_index,
                    node.time + self.get_latency(connection_type),
                    node.depth
                );
                
                if !visited.contains(&neighbor_node.hash) {
                    heap.push(neighbor_node);
                }
            }
        }
        
        let avg_time = total_time / nodes_reached as f64;
        pb.finish_with_message(format!("Complete!"));
        (max_time, min_time, avg_time, nodes_reached)
    }

    fn propagate_realtime(&self, start_node: usize) -> Result<(), io::Error> {
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();
        let start_time = Instant::now();
        heap.push(Node::new(start_node, 0.0, 0));
        
        let pb = ProgressBar::new(self.node_count as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos:>7}/{len:7} Nodes reached")
            .unwrap()
            .progress_chars("#>-"));
        
        let mut min_network_time: f64 = f64::MAX;
        let mut max_network_time: f64 = 0.0;
        let mut total_network_time: f64 = 0.0;
        let threshold = self.node_count - (self.node_count / 1000);
        let mut last_stretch_start: Option<Instant> = None;
        
        while let Some(node) = heap.pop() {
            if visited.contains(&node.hash) {
                continue;
            }
            
            visited.insert(node.hash);
            let nodes_reached = visited.len();
            pb.set_position(nodes_reached as u64);

            if nodes_reached == threshold {
                last_stretch_start = Some(Instant::now());
            }

            // Get all neighbors and their latencies
            let neighbors = self.get_neighbors(node.index);
            
            // Find the maximum latency for this batch of neighbors
            let max_latency = neighbors.iter()
                .map(|(_, conn_type)| self.get_latency(conn_type))
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);

            // Sleep only once for the maximum latency in this batch
            let sleep_micros = (max_latency * 1000.0) as u64;
            thread::sleep(Duration::from_micros(sleep_micros));

            // Process all neighbors
            for (neighbor_index, connection_type) in neighbors {
                let latency = self.get_latency(connection_type);
                let network_time = node.time + latency;
                
                min_network_time = min_network_time.min(network_time);
                max_network_time = max_network_time.max(network_time);
                total_network_time += network_time;
                
                let neighbor_node = Node::new(
                    neighbor_index,
                    network_time,
                    node.depth
                );
                
                if !visited.contains(&neighbor_node.hash) {
                    heap.push(neighbor_node);
                }
            }
        }
        
        let real_elapsed = start_time.elapsed();
        let avg_network_time = total_network_time / visited.len() as f64;
        let final_stretch_time = last_stretch_start.map(|t| t.elapsed().as_secs_f64());
        
        pb.finish_with_message(format!("Complete!"));
        println!("\nRealtime Simulation Complete:");
        println!("Nodes Reached: {}", visited.len());
        println!("Real Time: {:.2} seconds", real_elapsed.as_secs_f64());
        if let Some(stretch_time) = final_stretch_time {
            println!("Final 0.1% Time: {:.2} seconds", stretch_time);
        }
        println!("\nNetwork Times:");
        println!("  Fastest: {:.2} ms", min_network_time);
        println!("  Slowest: {:.2} ms", max_network_time);
        println!("  Average: {:.2} ms", avg_network_time);
        println!("Mode: {}", if self.fractal_mode { "Fractal" } else { "Flat" });
        
        Ok(())
    }

    fn get_neighbors(&self, index: usize) -> Vec<(usize, &'static str)> {
        if self.fractal_mode {
            // Convert from fractal neighbors format to standard format
            let node = Node::new(index, 0.0, self.calculate_depth(index));
            node.get_fractal_neighbors(self)
                .into_iter()
                .map(|(n, latency)| (n.index, self.get_connection_type(latency)))
                .collect()
        } else {
            // Original flat hex routing
            let mut neighbors = Vec::new();
            for i in 1..=6 {
                let neighbor_index = (index + i) % self.node_count;
                neighbors.push((neighbor_index, "local"));
            }
            neighbors
        }
    }

    fn calculate_depth(&self, index: usize) -> usize {
        if index == 0 {
            0
        } else {
            (index as f64).log(7.0).floor() as usize
        }
    }

    fn get_connection_type(&self, latency: f64) -> &'static str {
        if latency <= 0.1 { "local" }
        else if latency <= 1.0 { "regional" }
        else { "global" }
    }

    fn get_latency(&self, connection_type: &str) -> f64 {
        // Base latencies in milliseconds
        let base_latency = match connection_type {
            "local" => 1.0,     // 1ms for local
            "regional" => 10.0,   // 10ms for regional
            "global" => 120.0,    // 120ms for global
            _ => 1.0,
        };

        // Only add a tiny bit of jitter to simulate normal network variance
        let jitter = rand::thread_rng().gen_range(0.95..=1.05);  // ±5% variance
        base_latency * jitter
    }
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    let node_count = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10_000);
    let fractal_mode = args.contains(&"--fractal".to_string());
    let viz_mode = args.contains(&"--viz".to_string()) || args.contains(&"--vis".to_string());
    let realtime = args.contains(&"--realtime".to_string());

    let network = GlobalHexNetwork::new(node_count, fractal_mode);

    match (realtime, viz_mode) {
        (true, true) => {
            // Realtime with visualization
            run_with_visualization(&network, true)
        }
        (true, false) => {
            // Realtime without visualization
            network.propagate_realtime(0)
        }
        (false, true) => {
            // Normal simulation with visualization
            run_with_visualization(&network, false)
        }
        (false, false) => {
            // Normal simulation without visualization
            let (max_time, min_time, avg_time, nodes_reached) = network.propagate_signal(0);
            println!("\nSimulation Complete:");
            println!("Nodes Reached: {}", nodes_reached);
            println!("Fastest Node: {:.2} ms", min_time);
            println!("Slowest Node: {:.2} ms", max_time);
            println!("Average Time: {:.2} ms", avg_time);
            println!("Mode: {}", if fractal_mode { "Fractal" } else { "Flat" });
            Ok(())
        }
    }
}

struct VisualizationState {
    current_wave: usize,
    nodes_reached: usize,
    max_time: f64,
    last_latency: f64,
}

fn run_with_visualization(network: &GlobalHexNetwork, realtime: bool) -> Result<(), io::Error> {
    let (tx, rx) = channel::<VisualizationState>();
    
    // Clone network data needed for simulation
    let node_count = network.node_count;
    let fractal_mode = network.fractal_mode;
    let network_thread = GlobalHexNetwork::new(node_count, fractal_mode);
    let start_time = Instant::now();
    
    thread::spawn(move || {
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();
        heap.push(Node::new(0, 0.0, 0));
        let mut current_ring = 0;
        let mut current_time = 0.0;
        
        while let Some(node) = heap.pop() {
            if visited.contains(&node.hash) {
                continue;
            }
            
            visited.insert(node.hash);
            let ring = (visited.len() as f64).sqrt() as usize / 2;
            
            // Only sleep once per ring in realtime mode
            if realtime && ring > current_ring {
                thread::sleep(Duration::from_millis(5)); // Small fixed delay per ring
                current_ring = ring;
            }

            for (neighbor_index, connection_type) in network_thread.get_neighbors(node.index) {
                let latency = network_thread.get_latency(connection_type);
                current_time += latency;
                
                let neighbor_node = Node::new(neighbor_index, current_time, node.depth);
                if !visited.contains(&neighbor_node.hash) {
                    heap.push(neighbor_node);
                    
                    tx.send(VisualizationState {
                        current_wave: ring,
                        nodes_reached: visited.len(),
                        max_time: current_time,
                        last_latency: latency,
                    }).ok();
                }
            }
        }
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut visualizer = PropagationVisualizer::new(10);

    terminal.clear()?;

    loop {
        // Non-blocking check for updates
        if let Ok(state) = rx.try_recv() {
            visualizer.update_state(state);
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Percentage(100),
                ])
                .split(f.area());

            visualizer.draw(f, chunks[0]);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    terminal.clear()?;
    Ok(())
}
