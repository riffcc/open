use std::collections::{HashSet, BinaryHeap};
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
    widgets::{
        Block, 
        Borders, 
        canvas::{self, Canvas},
        Paragraph
    },
    layout::{Layout, Constraint, Direction, Rect},
};
use crossterm::event::{self, Event, KeyCode};
use std::sync::mpsc::channel;
use std::thread;
use indicatif::{ProgressBar, ProgressStyle};
use std::f64::consts::PI;

#[derive(Debug)]
pub struct SimulationResult {
    pub nodes_reached: usize,
    pub fastest_time: f64,
    pub slowest_time: f64,
    pub average_time: f64,
    pub mode: String,
}

struct HexPoint {
    pos: Point3D,
    ring: usize,
}

impl HexPoint {
    fn new(ring: usize, segment: usize) -> Self {
        let phi = (segment as f64) * PI / 6.0;
        let theta = (segment as f64) * PI / 4.0;
        let radius = ring as f64;
        Self {
            pos: Point3D {
                x: radius * theta.sin() * phi.cos() * 2.0,
                y: radius * theta.sin() * phi.sin() * 2.0,
                z: radius * theta.cos() * 2.0,
            },
            ring,
        }
    }
}

struct PropagationVisualizer {
    camera: Camera,
    current_wave: usize,
    hex_points: Vec<HexPoint>,
    current_time: f64,
    nodes_reached: usize,
    last_latency: f64,
    total_nodes: usize,
    max_depth: usize,
}

impl PropagationVisualizer {
    fn new(initial_rings: usize, total_nodes: usize) -> Self {
        let mut visualizer = Self {
            camera: Camera::new(),
            current_wave: 0,
            hex_points: Vec::new(),
            current_time: 0.0,
            nodes_reached: 1,
            last_latency: 0.0,
            total_nodes,
            max_depth: 0,
        };
        visualizer.ensure_ring_exists(initial_rings);
        visualizer
    }

    fn ensure_ring_exists(&mut self, ring: usize) {
        while self.hex_points.len() < ring * 12 + 1 {
            let current_ring = self.hex_points.len() / 12;
            for segment in 0..12 {
                self.hex_points.push(HexPoint::new(current_ring, segment));
            }
        }
    }

    fn calculate_zoom(&self) -> f64 {
        let max_radius = self.current_wave as f64;
        20.0 / max_radius.max(1.0)
    }

    fn draw(&self, f: &mut ratatui::Frame, area: Rect) {
        let zoom = self.calculate_zoom();
        
        let mut points_to_draw: Vec<_> = self.hex_points.iter()
            .map(|point| {
                let projected = self.camera.project(&point.pos);
                (projected, point)
            })
            .collect();
        
        points_to_draw.sort_by(|a, b| b.0.z.partial_cmp(&a.0.z).unwrap_or(Ordering::Equal));

        let canvas = Canvas::default()
            .block(Block::default().borders(Borders::ALL))
            .paint(|ctx| {
                let origin = self.camera.project(&Point3D { x: 0.0, y: 0.0, z: 0.0 });
                self.draw_node(ctx, origin.x, origin.y, "●", ratatui::style::Color::Green);

                for (projected, point) in points_to_draw.iter() {
                    let screen_x = projected.x * zoom;
                    let screen_y = projected.y * zoom;
                    
                    if screen_x.abs() > 20.0 || screen_y.abs() > 20.0 {
                        continue;
                    }

                    let depth = (point.ring as f64).log(12.0).floor() as usize;
                    if let Some((symbol, color)) = self.get_node_style(point.ring, depth) {
                        self.draw_node(ctx, screen_x, screen_y, symbol, color);
                        self.draw_structural_connection(ctx, point, zoom);
                    }
                }
            })
            .x_bounds([-20.0, 20.0])
            .y_bounds([-20.0, 20.0]);

        f.render_widget(canvas, area);
        self.draw_stats(f, area);
    }

    fn get_node_style(&self, ring: usize, depth: usize) -> Option<(&'static str, ratatui::style::Color)> {
        if ring > self.current_wave {
            return None;
        }

        if ring == self.current_wave {
            Some(("◆", match depth % 3 {
                0 => ratatui::style::Color::Rgb(255, 50, 50),   // First triangle layer: Red
                1 => ratatui::style::Color::Rgb(50, 255, 50),   // Second triangle layer: Green
                2 => ratatui::style::Color::Rgb(50, 50, 255),   // Ring layer: Blue
                _ => unreachable!()
            }))
        } else {
            // Show the network structure in dimmer colors
            Some(("·", match depth % 3 {
                0 => ratatui::style::Color::Rgb(100, 30, 30),   // Dim red for tri
                1 => ratatui::style::Color::Rgb(30, 100, 30),   // Dim green for tri
                2 => ratatui::style::Color::Rgb(30, 30, 100),   // Dim blue for ring
                _ => unreachable!()
            }))
        }
    }

    fn draw_node<'a>(&self, ctx: &mut canvas::Context<'a>, x: f64, y: f64, symbol: &'a str, color: ratatui::style::Color) {
        ctx.print(
            x, y,
            ratatui::text::Span::styled(
                symbol,
                ratatui::style::Style::default().fg(color)
            )
        );
    }

    fn draw_structural_connection<'a>(&self, ctx: &mut canvas::Context<'a>, point: &HexPoint, zoom: f64) {
        if point.ring > 0 && point.ring <= self.current_wave {
            let depth = point.ring % 3;
            
            // Calculate angle for connection visualization
            let parent_ring = point.ring / 7;
            let parent_angle = (point.ring % 6) as f64 * PI / 3.0;
            let parent_x = (parent_ring as f64 * parent_angle.cos()) * zoom;
            let parent_y = (parent_ring as f64 * parent_angle.sin()) * zoom;
            
            let angle = ((point.pos.y * zoom - parent_y) / 
                        (point.pos.x * zoom - parent_x)).atan();
            
            let connection_char = match (angle * 8.0 / PI) as i32 {
                -4|-3 => "╲",
                -2|-1 => "╱",
                0 => "┃",
                1|2 => "╲",
                3|4 => "╱",
                _ => "━",
            };
            
            let color = match depth {
                0 => ratatui::style::Color::Rgb(100, 30, 30),  // First triangle
                1 => ratatui::style::Color::Rgb(30, 100, 30),  // Second triangle
                2 => ratatui::style::Color::Rgb(30, 30, 100),  // Ring
                _ => unreachable!()
            };
            
            self.draw_node(
                ctx,
                (point.pos.x * zoom + parent_x) / 2.0,
                (point.pos.y * zoom + parent_y) / 2.0,
                connection_char,
                color
            );
        }
    }

    fn draw_stats(&self, f: &mut ratatui::Frame, area: Rect) {
        let stats = format!(
            "Network Time: {:.2}ms | Nodes: {}/{} | Wave Front: {} | Tree Depth: {} | Latency: {:.2}ms",
            self.current_time,
            self.nodes_reached,
            self.total_nodes,
            self.current_wave,
            self.max_depth,
            self.last_latency
        );

        let stats_widget = Paragraph::new(stats)
            .block(Block::default())
            .style(ratatui::style::Style::default());

        let stats_area = Rect::new(
            area.x + 1,
            area.y + area.height - 1,
            area.width - 2,
            1
        );

        f.render_widget(stats_widget, stats_area);
    }

    fn update_state(&mut self, state: VisualizationState) {
        self.current_wave = state.current_wave;
        self.nodes_reached = state.nodes_reached;
        self.current_time = state.max_time;
        self.last_latency = state.last_latency;
        self.max_depth = state.max_depth;
        self.ensure_ring_exists(self.current_wave);
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
        let layer_size = 3_usize.pow(current_depth as u32);
        
        // Each layer connects to exactly 3 neighbors in different orientations
        let triangle_offsets: Vec<usize> = match current_depth % 3 {
            0 => vec![
                1,                  // Right
                layer_size / 3,     // Upper
                2 * layer_size / 3  // Left
            ],
            1 => vec![
                layer_size / 6,     // Lower right
                layer_size / 2,     // Upper
                5 * layer_size / 6  // Lower left
            ],
            2 => vec![
                layer_size / 4,     // Right diagonal
                layer_size / 2,     // Center
                3 * layer_size / 4  // Left diagonal
            ],
            _ => unreachable!()
        };

        println!("Node {} at depth {} connecting with offsets {:?}", 
                self.index, current_depth, triangle_offsets);

        // Connect to exactly 3 neighbors in this layer's orientation
        for &offset in &triangle_offsets {
            let neighbor_index = (self.index + offset) % network.node_count;
            if neighbor_index != self.index && neighbor_index < network.node_count {
                neighbors.push((
                    Node::new(neighbor_index, self.time, current_depth),
                    0.1  // Fast local propagation
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

#[derive(Clone)]
struct GlobalHexNetwork {
    node_count: usize,
    fractal_mode: bool,
    queue: BinaryHeap<Node>,
}

#[derive(Clone)]
struct NetworkEvent {
    node: Node,
    arrival_time: f64,
}

impl PartialEq for NetworkEvent {
    fn eq(&self, other: &Self) -> bool {
        self.arrival_time == other.arrival_time
    }
}

impl Eq for NetworkEvent {}

impl PartialOrd for NetworkEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.arrival_time.partial_cmp(&self.arrival_time)
    }
}

impl Ord for NetworkEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl GlobalHexNetwork {
    fn new(node_count: usize, fractal_mode: bool) -> Self {
        let mut queue = BinaryHeap::new();
        queue.push(Node::new(0, 0.0, 0));
        
        Self {
            node_count,
            fractal_mode,
            queue,
        }
    }

    fn get_max_depth(&self) -> usize {
        if self.fractal_mode {
            (self.node_count as f64).log(20.0).ceil() as usize  // Changed to 20 for 3D
        } else {
            1
        }
    }

    fn calculate_depth(&self, index: usize) -> usize {
        if index == 0 {
            0
        } else {
            (index as f64).log(20.0).floor() as usize  // Changed to 20 for 3D
        }
    }

    fn get_neighbors(&self, index: usize) -> Vec<(usize, &'static str)> {
        if self.fractal_mode {
            if index >= self.node_count {
                return Vec::new();
            }

            let node = Node::new(index, 0.0, self.calculate_depth(index));
            node.get_fractal_neighbors(self)
                .into_iter()
                .filter(|(n, _)| n.index < self.node_count)
                .map(|(n, latency)| (n.index, self.get_connection_type(latency)))
                .collect()
        } else {
            // Non-fractal mode: connect to immediate neighbors in 3D hex grid
            let mut neighbors = Vec::new();
            
            // Same plane connections
            for i in 1..=12 {
                let neighbor_index = (index + i) % self.node_count;
                neighbors.push((neighbor_index, "local"));
            }
            
            // Upper and lower plane connections
            let layer_size = (self.node_count as f64).cbrt().ceil() as usize;
            let layer_offset = layer_size * layer_size;
            
            // Connect to 4 nodes in upper layer
            if index >= layer_offset {
                for i in 0..4 {
                    let neighbor_index = index - layer_offset + i;
                    if neighbor_index < self.node_count {
                        neighbors.push((neighbor_index, "regional"));
                    }
                }
            }
            
            // Connect to 4 nodes in lower layer
            if index + layer_offset < self.node_count {
                for i in 0..4 {
                    let neighbor_index = index + layer_offset + i;
                    if neighbor_index < self.node_count {
                        neighbors.push((neighbor_index, "regional"));
                    }
                }
            }
            
            neighbors
        }
    }

    fn get_connection_type(&self, latency: f64) -> &'static str {
        if latency <= 0.1 { "local" }
        else if latency <= 1.0 { "regional" }
        else { "global" }
    }

    fn get_latency(&self, connection_type: &str) -> f64 {
        let base_latency = match connection_type {
            "ring" => 2.0,     // POP-to-POP in ring (~2ms)
            "local" => 0.5,    // Suburb: 0.5ms (local fiber/mesh network)
            "hex" => 5.0,      // Country: 5ms (national backbone)
            "regional" => 50.0, // Continent: 50ms (cross-continent)
            "global" => 150.0,  // Globe: 150ms (transoceanic cables)
            _ => 0.5,
        };

        // Real networks have variable latency
        let jitter = rand::thread_rng().gen_range(0.8..=1.2);  // 20% jitter
        base_latency * jitter
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
        let mut event_queue = BinaryHeap::new();
        let start_time = Instant::now();
        
        event_queue.push(NetworkEvent {
            node: Node::new(start_node, 0.0, 0),
            arrival_time: 0.0,
        });

        let pb = ProgressBar::new(self.node_count as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos:>7}/{len:7} Nodes reached")
            .unwrap()
            .progress_chars("#>-"));
        
        let mut min_network_time: f64 = f64::MAX;
        let mut max_network_time: f64 = 0.0;
        let mut total_network_time: f64 = 0.0;
        let mut nodes_processed = 0;
        
        while let Some(event) = event_queue.pop() {
            if visited.contains(&event.node.hash) {
                continue;
            }

            min_network_time = min_network_time.min(event.arrival_time * 1000.0);
            max_network_time = max_network_time.max(event.arrival_time * 1000.0);
            total_network_time += event.arrival_time * 1000.0;
            nodes_processed += 1;

            let sleep_time = (event.arrival_time - start_time.elapsed().as_secs_f64()).max(0.0);
            if sleep_time > 0.0 {
                thread::sleep(Duration::from_secs_f64(sleep_time));
            }

            visited.insert(event.node.hash);
            pb.set_position(visited.len() as u64);

            for (neighbor_index, connection_type) in self.get_neighbors(event.node.index) {
                let latency = self.get_latency(connection_type);
                let arrival_time = event.arrival_time + (latency / 1000.0);
                
                let neighbor_node = Node::new(
                    neighbor_index,
                    arrival_time,
                    event.node.depth
                );

                if !visited.contains(&neighbor_node.hash) {
                    event_queue.push(NetworkEvent {
                        node: neighbor_node,
                        arrival_time,
                    });
                }
            }
        }

        let real_elapsed = start_time.elapsed();
        let avg_network_time = total_network_time / nodes_processed as f64;
        
        pb.finish_with_message(format!("Complete!"));
        println!("\nRealtime Simulation Complete:");
        println!("Nodes Reached: {}", visited.len());
        println!("Real Time: {:.2} seconds", real_elapsed.as_secs_f64());
        println!("\nNetwork Times:");
        println!("  Fastest: {:.2} ms", min_network_time);
        println!("  Slowest: {:.2} ms", max_network_time);
        println!("  Average: {:.2} ms", avg_network_time);
        println!("Mode: {}", if self.fractal_mode { "Fractal" } else { "Flat" });
        
        Ok(())
    }

    // Add timing metrics to the simulation
    fn simulate(&mut self) -> SimulationResult {
        let mut nodes_reached = HashSet::new();
        let mut times = Vec::new();
        let mut current_wave_size = 0;
        let mut last_wave_time = 0.0;

        while let Some((node, time)) = self.queue.pop() {
            if nodes_reached.insert(node.index) {
                current_wave_size += 1;
                times.push(time);

                // Measure wave propagation speed
                if time - last_wave_time > 1.0 {
                    println!("Wave at time {:.2}ms reached {} nodes", 
                        time, current_wave_size);
                    current_wave_size = 0;
                    last_wave_time = time;
                }

                for (neighbor, latency) in self.get_neighbors(&node) {
                    if !nodes_reached.contains(&neighbor.index) {
                        self.queue.push(neighbor, time + latency);
                    }
                }
            }
        }

        SimulationResult {
            nodes_reached: nodes_reached.len(),
            fastest_time: times.iter().copied().fold(f64::INFINITY, f64::min),
            slowest_time: times.iter().copied().fold(0.0, f64::max),
            average_time: times.iter().sum::<f64>() / times.len() as f64,
            mode: self.mode.clone(),
        }
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
            run_with_visualization(network.clone(), true)
        }
        (true, false) => {
            network.propagate_realtime(0)
        }
        (false, true) => {
            run_with_visualization(network.clone(), false)
        }
        (false, false) => {
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
    max_depth: usize,
}

struct PropagationEvent {
    state: VisualizationState,
    is_complete: bool,
}

fn run_with_visualization(network: GlobalHexNetwork, _realtime: bool) -> Result<(), io::Error> {
    let (mut tx, mut rx) = channel::<PropagationEvent>();
    let node_count = network.node_count;
    
    let network_sim = network.clone();
    
    let mut _simulation_thread = thread::spawn(move || {
        let mut visited = HashSet::with_capacity(node_count);
        let mut event_queue = BinaryHeap::new();
        let mut arrival_times = Vec::with_capacity(node_count);
        let mut last_viz_update = Instant::now();
        let mut max_depth = 0;
        let mut last_latency = 0.0;
        
        let origin = Node::new(0, 0.0, 0);
        visited.insert(origin.hash);
        arrival_times.push(0.0);
        event_queue.push(NetworkEvent {
            node: origin,
            arrival_time: 0.0,
        });

        tx.send(PropagationEvent {
            state: VisualizationState {
                current_wave: 0,
                nodes_reached: 1,
                max_time: 0.0,
                last_latency: 0.0,
                max_depth: 0,
            },
            is_complete: false,
        }).ok();

        while let Some(event) = event_queue.pop() {
            if visited.len() >= node_count {
                break;
            }

            for (neighbor_index, connection_type) in network_sim.get_neighbors(event.node.index) {
                if neighbor_index >= node_count {
                    continue;
                }

                last_latency = network_sim.get_latency(connection_type);
                let arrival_time = event.arrival_time + (last_latency / 1000.0);
                
                let neighbor_node = Node::new(
                    neighbor_index,
                    arrival_time,
                    event.node.depth + 1,
                );

                if !visited.contains(&neighbor_node.hash) {
                    visited.insert(neighbor_node.hash);
                    arrival_times.push(arrival_time);
                    
                    if neighbor_node.depth > max_depth {
                        max_depth = neighbor_node.depth;
                    }

                    event_queue.push(NetworkEvent {
                        node: neighbor_node,
                        arrival_time,
                    });

                    if last_viz_update.elapsed() > Duration::from_millis(8) {
                        let times: Vec<f64> = arrival_times.iter()
                            .map(|&t| t * 1000.0)
                            .collect();
                        
                        tx.send(PropagationEvent {
                            state: VisualizationState {
                                current_wave: max_depth,
                                nodes_reached: visited.len(),
                                max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                                last_latency,
                                max_depth,
                            },
                            is_complete: false,
                        }).ok();
                        last_viz_update = Instant::now();
                    }
                }
            }
        }

        let times: Vec<f64> = arrival_times.iter()
            .map(|&t| t * 1000.0)
            .collect();
        
        tx.send(PropagationEvent {
            state: VisualizationState {
                current_wave: max_depth,
                nodes_reached: visited.len(),
                max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                last_latency,
                max_depth,
            },
            is_complete: true,
        }).ok();
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
    )?;

    let mut visualizer = PropagationVisualizer::new(10, node_count);
    let mut paused = false;

    let result = loop {
        if !paused {
            if let Ok(event) = rx.try_recv() {
                visualizer.update_state(event.state);
                if event.is_complete {
                    paused = true;
                }
            }
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Percentage(95),
                    Constraint::Percentage(5),
                ])
                .split(f.area());

            visualizer.draw(f, chunks[0]);

            let help_text = if paused {
                "Press [R] to restart, [Enter/Esc] or [Q] to exit, [Space] to continue"
            } else {
                "Press [Q] to quit, [Space] to pause"
            };
            let help = Paragraph::new(help_text)
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(help, chunks[1]);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') if !paused => break Ok(()),
                    KeyCode::Char('r') if paused => {
                        paused = false;
                        visualizer = PropagationVisualizer::new(10, node_count);
                        
                        let (new_tx, new_rx) = channel();
                        tx = new_tx;
                        rx = new_rx;
                        
                        let network_sim = network.clone();
                        _simulation_thread = thread::spawn(move || {
                            let mut visited = HashSet::with_capacity(node_count);
                            let mut event_queue = BinaryHeap::new();
                            let mut arrival_times = Vec::with_capacity(node_count);
                            let mut last_viz_update = Instant::now();
                            let mut max_depth = 0;
                            let mut last_latency = 0.0;
                            
                            let origin = Node::new(0, 0.0, 0);
                            visited.insert(origin.hash);
                            arrival_times.push(0.0);
                            event_queue.push(NetworkEvent {
                                node: origin,
                                arrival_time: 0.0,
                            });

                            tx.send(PropagationEvent {
                                state: VisualizationState {
                                    current_wave: 0,
                                    nodes_reached: 1,
                                    max_time: 0.0,
                                    last_latency: 0.0,
                                    max_depth: 0,
                                },
                                is_complete: false,
                            }).ok();

                            while let Some(event) = event_queue.pop() {
                                if visited.len() >= node_count {
                                    break;
                                }

                                for (neighbor_index, connection_type) in network_sim.get_neighbors(event.node.index) {
                                    if neighbor_index >= node_count {
                                        continue;
                                    }

                                    last_latency = network_sim.get_latency(connection_type);
                                    let arrival_time = event.arrival_time + (last_latency / 1000.0);
                                    
                                    let neighbor_node = Node::new(
                                        neighbor_index,
                                        arrival_time,
                                        event.node.depth + 1,
                                    );

                                    if !visited.contains(&neighbor_node.hash) {
                                        visited.insert(neighbor_node.hash);
                                        arrival_times.push(arrival_time);
                                        
                                        if neighbor_node.depth > max_depth {
                                            max_depth = neighbor_node.depth;
                                        }

                                        event_queue.push(NetworkEvent {
                                            node: neighbor_node,
                                            arrival_time,
                                        });

                                        if last_viz_update.elapsed() > Duration::from_millis(8) {
                                            let times: Vec<f64> = arrival_times.iter()
                                                .map(|&t| t * 1000.0)
                                                .collect();
                                            
                                            tx.send(PropagationEvent {
                                                state: VisualizationState {
                                                    current_wave: max_depth,
                                                    nodes_reached: visited.len(),
                                                    max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                                                    last_latency,
                                                    max_depth,
                                                },
                                                is_complete: false,
                                            }).ok();
                                            last_viz_update = Instant::now();
                                        }
                                    }
                                }
                            }

                            let times: Vec<f64> = arrival_times.iter()
                                .map(|&t| t * 1000.0)
                                .collect();
                            
                            tx.send(PropagationEvent {
                                state: VisualizationState {
                                    current_wave: max_depth,
                                    nodes_reached: visited.len(),
                                    max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                                    last_latency,
                                    max_depth,
                                },
                                is_complete: true,
                            }).ok();
                        });
                    },
                    KeyCode::Char(' ') => paused = !paused,
                    KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc if paused => break Ok(()),
                    KeyCode::Left => visualizer.camera.rotation_y -= 0.1,
                    KeyCode::Right => visualizer.camera.rotation_y += 0.1,
                    KeyCode::Up => visualizer.camera.rotation_x -= 0.1,
                    KeyCode::Down => visualizer.camera.rotation_x += 0.1,
                    KeyCode::Char('+') => visualizer.camera.zoom *= 1.1,
                    KeyCode::Char('-') => visualizer.camera.zoom /= 1.1,
                    _ => {}
                }
            }
        }
    };

    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show,
    )?;
    crossterm::terminal::disable_raw_mode()?;

    result
}

struct Camera {
    rotation_x: f64,
    rotation_y: f64,
    zoom: f64,
}

impl Camera {
    fn new() -> Self {
        Self {
            rotation_x: 0.0,
            rotation_y: 0.0,
            zoom: 1.0,
        }
    }

    fn project(&self, point: &Point3D) -> Point2D {
        let (sin_x, cos_x) = self.rotation_x.sin_cos();
        let (sin_y, cos_y) = self.rotation_y.sin_cos();

        let x1 = point.x * cos_y - point.z * sin_y;
        let z1 = point.x * sin_y + point.z * cos_y;

        let y2 = point.y * cos_x - z1 * sin_x;
        let z2 = point.y * sin_x + z1 * cos_x;

        let depth = 5.0;
        let scale = depth / (depth + z2);

        Point2D {
            x: x1 * scale * self.zoom,
            y: y2 * scale * self.zoom,
            z: z2,
        }
    }
}

struct Point3D {
    x: f64,
    y: f64,
    z: f64,
}

struct Point2D {
    x: f64,
    y: f64,
    z: f64,
}
