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
    widgets::{Block, Borders, Paragraph, canvas::Canvas},
    layout::{Layout, Constraint, Direction, Rect},
    style::{Style, Color},
};
use crossterm::event::{self, Event, KeyCode};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

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
    waves: Vec<WaveFront>,
    current_wave: usize,
    start_time: Instant,
    hex_points: Vec<HexPoint>,
}

impl PropagationVisualizer {
    fn new(initial_rings: usize) -> Self {
        let capacity = initial_rings * 6 + 1; // Preallocate exact space needed
        let mut hex_points = Vec::with_capacity(capacity);
        
        // Generate points lazily
        hex_points.push(HexPoint { x: 0.0, y: 0.0, ring: 0 });
        
        Self {
            waves: Vec::with_capacity(initial_rings),
            current_wave: 0,
            start_time: Instant::now(),
            hex_points,
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

    fn add_wave(&mut self, ring: usize, nodes: usize, time: f64) {
        self.waves.push(WaveFront {
            ring,
            nodes,
            time,
            start_time: Instant::now(),
        });
        self.current_wave = ring;
    }

    fn calculate_zoom(&self) -> f64 {
        let max_radius = self.current_wave as f64;
        20.0 / max_radius.max(1.0) // Automatically zoom out as waves expand
    }

    fn draw(&self, f: &mut ratatui::Frame, area: Rect) {
        let elapsed = self.start_time.elapsed().as_secs_f64() * 1000.0;
        let zoom = self.calculate_zoom();
        
        let canvas = Canvas::default()
            .block(Block::default().title("Propagation Waves").borders(Borders::ALL))
            .paint(|ctx| {
                for point in &self.hex_points {
                    if point.ring > self.current_wave + 2 {
                        continue;
                    }
                    
                    let screen_x = point.x * zoom;
                    let screen_y = point.y * zoom;
                    
                    if screen_x.abs() > 20.0 || screen_y.abs() > 20.0 {
                        continue;
                    }

                    if point.ring as f64 * 5.0 <= elapsed {
                        ctx.print(screen_x, screen_y, "⬢");
                    } else if point.ring as f64 * 5.0 <= elapsed + 5.0 {
                        ctx.print(screen_x, screen_y, "⬡");
                    }
                }
            })
            .x_bounds([-20.0, 20.0])
            .y_bounds([-20.0, 20.0]);

        f.render_widget(canvas, area);
    }

    fn update_state(&mut self, state: VisualizationState) {
        self.current_wave = state.current_wave;
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

    fn propagate_with_visualization(&self, _start_node: usize) -> (f64, usize, PropagationVisualizer) {
        let mut visualizer = PropagationVisualizer::new((self.node_count as f64).sqrt() as usize);
        let mut total_nodes = 1;
        let mut current_time = 0.0;
        
        // Add center node
        visualizer.add_wave(0, 1, 0.0);
        
        let mut ring = 0;
        while total_nodes < self.node_count {
            ring += 1;
            let nodes_in_ring = ring * 6;
            current_time += self.get_latency("local");
            
            visualizer.add_wave(ring, nodes_in_ring, current_time);
            total_nodes += nodes_in_ring;
            
            // Small delay to make visualization visible
            std::thread::sleep(Duration::from_millis(50));
        }
        
        (current_time, total_nodes, visualizer)
    }
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    let node_count = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10_000);
    let fractal_mode = args.contains(&"--fractal".to_string());
    let viz_mode = args.contains(&"--viz".to_string());

    let network = GlobalHexNetwork::new(node_count, fractal_mode);

    if viz_mode {
        // Run with visualization
        run_with_visualization(&network)
    } else {
        // Run at full speed
        let (max_time, nodes_reached) = network.propagate_signal(0);
        println!("Simulation Complete:");
        println!("Nodes Reached: {}", nodes_reached);
        println!("Max Time: {:.2} ms", max_time);
        println!("Mode: {}", if fractal_mode { "Fractal" } else { "Flat" });
        Ok(())
    }
}

struct VisualizationState {
    current_wave: usize,
    nodes_reached: usize,
    max_time: f64,
}

fn run_with_visualization(network: &GlobalHexNetwork) -> Result<(), io::Error> {
    let (tx, rx) = channel::<VisualizationState>();
    
    // Clone network data needed for simulation
    let node_count = network.node_count;
    let fractal_mode = network.fractal_mode;
    let network_thread = GlobalHexNetwork::new(node_count, fractal_mode);
    
    // Run simulation in separate thread with owned data
    thread::spawn(move || {
        let (max_time, nodes_reached) = network_thread.propagate_signal(0);
        if nodes_reached % 100 == 0 {
            tx.send(VisualizationState {
                current_wave: (nodes_reached as f64).sqrt() as usize / 6,
                nodes_reached,
                max_time,
            }).ok();
        }
    });

    // Visualization loop
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut visualizer = PropagationVisualizer::new(10); // Start small

    loop {
        // Non-blocking check for updates
        if let Ok(state) = rx.try_recv() {
            visualizer.update_state(state);
        }

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

            let title = format!("GlobalHexNetwork Simulation");
            let block = Block::default().title(title).borders(Borders::ALL);
            f.render_widget(block, chunks[0]);

            visualizer.draw(f, chunks[1]);

            let stats = Paragraph::new(format!(
                "Nodes Reached: {}\nMax Time: {:.2} ms\nRings: {}",
                visualizer.current_wave * 6,
                visualizer.waves[visualizer.current_wave].time,
                visualizer.current_wave
            ))
            .block(Block::default().title("Propagation Stats").borders(Borders::ALL));
            f.render_widget(stats, chunks[2]);
        })?;

        // Check for quit with timeout
        if event::poll(Duration::from_millis(16))? { // ~60 FPS max
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }
    Ok(())
}
