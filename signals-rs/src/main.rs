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
    total_nodes: usize,
    max_depth: usize,
}

impl PropagationVisualizer {
    fn new(initial_rings: usize, total_nodes: usize) -> Self {
        let mut visualizer = Self {
            current_wave: 0,
            start_time: Instant::now(),
            hex_points: Vec::new(),  // Start empty
            current_time: 0.0,
            nodes_reached: 1,  // Start with origin counted
            last_latency: 0.0,
            total_nodes,
            max_depth: 0,
        };
        visualizer.ensure_ring_exists(initial_rings);  // Pre-populate initial rings
        visualizer
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
        
        let canvas = Canvas::default()
            .block(Block::default().borders(Borders::ALL))
            .paint(|ctx| {
                // Always draw the origin node
                self.draw_node(ctx, 0.0, 0.0, "●", ratatui::style::Color::Green);

                // Draw rest of the nodes
                for point in &self.hex_points {
                    let screen_x = point.x * zoom;
                    let screen_y = point.y * zoom;
                    
                    if screen_x.abs() > 20.0 || screen_y.abs() > 20.0 {
                        continue;
                    }

                    let depth = (point.ring as f64).log(7.0).floor() as usize;
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
            // Active wave front - use rainbow colors
            Some(("◆", match depth % 6 {
                0 => ratatui::style::Color::Rgb(255, 50, 50),   // Red
                1 => ratatui::style::Color::Rgb(255, 200, 50),  // Orange
                2 => ratatui::style::Color::Rgb(50, 255, 50),   // Green
                3 => ratatui::style::Color::Rgb(50, 200, 255),  // Cyan
                4 => ratatui::style::Color::Rgb(150, 50, 255),  // Purple
                _ => ratatui::style::Color::Rgb(255, 50, 255),  // Magenta
            }))
        } else {
            // Already visited nodes - use depth-based coloring
            let intensity = ((255.0 * 0.7) - (depth as f64 * 20.0)).max(30.0) as u8;
            Some(("·", ratatui::style::Color::Rgb(
                intensity / 2,
                intensity / 3,
                intensity
            )))
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
        if point.ring > 0 && point.ring <= self.current_wave {  // Only draw connections up to current wave
            let parent_ring = point.ring / 7;
            let parent_angle = (point.ring % 6) as f64 * std::f64::consts::PI / 3.0;
            let parent_x = (parent_ring as f64 * parent_angle.cos()) * zoom;
            let parent_y = (parent_ring as f64 * parent_angle.sin()) * zoom;
            
            let dx = point.x * zoom - parent_x;
            let dy = point.y * zoom - parent_y;
            
            // Fancy connection characters based on angle
            let connection_char = match ((dy/dx).atan() * 8.0 / std::f64::consts::PI) as i32 {
                -4|-3 => "╲",
                -2|-1 => "╱",
                0 => "┃",
                1|2 => "╲",
                3|4 => "╱",
                _ => "━",
            };
            
            // Color based on depth for better visibility
            let depth = (point.ring as f64).log(7.0).floor() as u8;
            let color = ratatui::style::Color::Rgb(
                155 + depth * 20,
                155 - depth * 30,
                155 + depth * 40
            );
            
            self.draw_node(
                ctx,
                (point.x * zoom + parent_x) / 2.0,
                (point.y * zoom + parent_y) / 2.0,
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
        
        // Early exit if we're beyond max depth
        if current_depth > network.max_depth {
            return Vec::new();
        }

        let cluster_size = 7_usize.pow(current_depth as u32);
        
        // Local cluster connections
        let cluster_start = (self.index / cluster_size) * cluster_size;
        for i in 1..=6 {
            let neighbor_index = cluster_start + ((self.index - cluster_start + i) % cluster_size);
            if neighbor_index < network.node_count {
                neighbors.push((
                    Node::new(neighbor_index, self.time, current_depth),
                    0.1
                ));
            }
        }

        // Parent connection (if not root)
        if self.index > 0 {
            let parent_index = self.index / 7;
            if parent_index < network.node_count {
                neighbors.push((
                    Node::new(parent_index, self.time, current_depth.saturating_sub(1)),
                    1.0
                ));
            }
        }

        // Child connections (only if we won't exceed node count)
        let child_base = self.index * 7 + 1;
        if child_base < network.node_count {
            for i in 0..6 {
                let child_index = child_base + i;
                if child_index < network.node_count {
                    neighbors.push((
                        Node::new(child_index, self.time, current_depth + 1),
                        10.0
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

#[derive(Clone)]
struct GlobalHexNetwork {
    node_count: usize,
    max_depth: usize,
    latencies: HashMap<&'static str, f64>,
    fractal_mode: bool,
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
        // Earlier times should be higher priority
        other.arrival_time.partial_cmp(&self.arrival_time)
    }
}

impl Ord for NetworkEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // Unwrap is safe because we're only comparing f64s
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl GlobalHexNetwork {
    fn new(target_nodes: usize, fractal_mode: bool) -> Self {
        let (node_count, max_depth) = if fractal_mode {
            // Calculate the depth needed for the target number of nodes
            let mut total_nodes = 0;
            let mut depth = 0;
            
            while total_nodes < target_nodes {
                total_nodes += 7_usize.pow(depth as u32);
                if total_nodes >= target_nodes {
                    break;
                }
                depth += 1;
            }
            
            (total_nodes.min(target_nodes), depth)
        } else {
            (target_nodes, 1)
        };
        
        let mut latencies = HashMap::new();
        latencies.insert("local", 0.1);
        latencies.insert("regional", 1.0);
        latencies.insert("global", 5.0);

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
        let mut event_queue = BinaryHeap::new();
        let start_time = Instant::now();
        
        // Initial event at time 0
        event_queue.push(NetworkEvent {
            node: Node::new(start_node, 0.0, 0),
            arrival_time: 0.0,
        });

        let pb = ProgressBar::new(self.node_count as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos:>7}/{len:7} Nodes reached")
            .unwrap()
            .progress_chars("#>-"));
        
        // Explicitly type our floats as f64
        let mut min_network_time: f64 = f64::MAX;
        let mut max_network_time: f64 = 0.0;
        let mut total_network_time: f64 = 0.0;
        let mut nodes_processed = 0;
        
        while let Some(event) = event_queue.pop() {
            if visited.contains(&event.node.hash) {
                continue;
            }

            // Track network times (in milliseconds)
            min_network_time = min_network_time.min(event.arrival_time * 1000.0);
            max_network_time = max_network_time.max(event.arrival_time * 1000.0);
            total_network_time += event.arrival_time * 1000.0;
            nodes_processed += 1;

            // Only sleep until this event's time in realtime mode
            let sleep_time = (event.arrival_time - start_time.elapsed().as_secs_f64()).max(0.0);
            if sleep_time > 0.0 {
                thread::sleep(Duration::from_secs_f64(sleep_time));
            }

            visited.insert(event.node.hash);
            pb.set_position(visited.len() as u64);

            // Schedule neighbor events
            for (neighbor_index, connection_type) in self.get_neighbors(event.node.index) {
                let latency = self.get_latency(connection_type);
                let arrival_time = event.arrival_time + (latency / 1000.0); // Convert ms to seconds
                
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

    fn get_neighbors(&self, index: usize) -> Vec<(usize, &'static str)> {
        if self.fractal_mode {
            // Early return if we're already at or past the node limit
            if index >= self.node_count {
                return Vec::new();
            }

            let node = Node::new(index, 0.0, self.calculate_depth(index));
            node.get_fractal_neighbors(self)
                .into_iter()
                .filter(|(n, _)| n.index < self.node_count)  // Extra safety filter
                .map(|(n, latency)| (n.index, self.get_connection_type(latency)))
                .collect()
        } else {
            // Original flat hex routing with node count limit
            let mut neighbors = Vec::new();
            for i in 1..=6 {
                let neighbor_index = (index + i) % self.node_count;
                if neighbor_index < self.node_count {
                    neighbors.push((neighbor_index, "local"));
                }
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
            run_with_visualization(network.clone(), true)
        }
        (true, false) => {
            // Realtime without visualization
            network.propagate_realtime(0)
        }
        (false, true) => {
            // Normal simulation with visualization
            run_with_visualization(network.clone(), false)
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
    min_time: f64,
    avg_time: f64,
    last_latency: f64,
    max_depth: usize,
}

// Add this new struct to handle sync events
struct PropagationEvent {
    state: VisualizationState,
    is_complete: bool,
}

fn run_with_visualization(network: GlobalHexNetwork, _realtime: bool) -> Result<(), io::Error> {
    let (mut tx, mut rx) = channel::<PropagationEvent>();
    let node_count = network.node_count;
    
    // Create a separate clone for the simulation thread
    let network_sim = network.clone();
    
    // Spawn simulation thread
    let mut simulation_thread = thread::spawn(move || {
        let mut visited = HashSet::with_capacity(node_count);
        let mut event_queue = BinaryHeap::new();
        let mut arrival_times = Vec::with_capacity(node_count);
        let mut last_viz_update = Instant::now();
        let mut max_depth = 0;
        let mut last_latency = 0.0;  // Track last latency
        
        // Create and count origin node
        let origin = Node::new(0, 0.0, 0);
        visited.insert(origin.hash);
        arrival_times.push(0.0);
        event_queue.push(NetworkEvent {
            node: origin,
            arrival_time: 0.0,
        });

        // Force initial visualization
        tx.send(PropagationEvent {
            state: VisualizationState {
                current_wave: 0,
                nodes_reached: 1,
                max_time: 0.0,
                min_time: 0.0,
                avg_time: 0.0,
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

                let current_latency = network_sim.get_latency(connection_type);
                last_latency = current_latency;  // Store current latency
                let arrival_time = event.arrival_time + (current_latency / 1000.0);
                
                let neighbor_node = Node::new(
                    neighbor_index,
                    arrival_time,
                    event.node.depth + 1,
                );

                if !visited.contains(&neighbor_node.hash) {
                    visited.insert(neighbor_node.hash);
                    arrival_times.push(arrival_time);
                    
                    // Update max_depth when processing nodes
                    if neighbor_node.depth > max_depth {
                        max_depth = neighbor_node.depth;
                    }

                    event_queue.push(NetworkEvent {
                        node: neighbor_node,
                        arrival_time,
                    });

                    // Update visualization with progress
                    if last_viz_update.elapsed() > Duration::from_millis(8) {
                        let times: Vec<f64> = arrival_times.iter()
                            .map(|&t| t * 1000.0)
                            .collect();
                        
                        tx.send(PropagationEvent {
                            state: VisualizationState {
                                current_wave: max_depth,
                                nodes_reached: visited.len(),
                                max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                                min_time: times.iter().fold(f64::INFINITY, |a: f64, &b| a.min(b)),
                                avg_time: times.iter().sum::<f64>() / times.len() as f64,
                                last_latency,  // Use tracked latency
                                max_depth,
                            },
                            is_complete: false,
                        }).ok();
                        last_viz_update = Instant::now();
                    }
                }
            }
        }

        // Send final state
        let times: Vec<f64> = arrival_times.iter()
            .map(|&t| t * 1000.0)
            .collect();
        
        tx.send(PropagationEvent {
            state: VisualizationState {
                current_wave: max_depth,
                nodes_reached: visited.len(),
                max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                min_time: times.iter().fold(f64::INFINITY, |a: f64, &b| a.min(b)),
                avg_time: times.iter().sum::<f64>() / times.len() as f64,
                last_latency,  // Use tracked latency
                max_depth,
            },
            is_complete: true,
        }).ok();
    });

    // Set up terminal
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Enter alternate screen and hide cursor
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
    )?;

    let mut visualizer = PropagationVisualizer::new(10, node_count);
    let mut simulation_complete = false;
    let mut paused = false;

    // Main visualization loop
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

            // Draw controls help
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
                        // Restart simulation
                        paused = false;
                        visualizer = PropagationVisualizer::new(10, node_count);
                        
                        // Create new channel and spawn new simulation
                        let (new_tx, new_rx) = channel();
                        tx = new_tx;
                        rx = new_rx;
                        
                        // Create new simulation thread with fresh network clone
                        let network_sim = network.clone();
                        simulation_thread = thread::spawn(move || {
                            let mut visited = HashSet::with_capacity(node_count);
                            let mut event_queue = BinaryHeap::new();
                            let mut arrival_times = Vec::with_capacity(node_count);
                            let mut last_viz_update = Instant::now();
                            let mut max_depth = 0;
                            let mut last_latency = 0.0;
                            
                            // Create and count origin node
                            let origin = Node::new(0, 0.0, 0);
                            visited.insert(origin.hash);
                            arrival_times.push(0.0);
                            event_queue.push(NetworkEvent {
                                node: origin,
                                arrival_time: 0.0,
                            });

                            // Force initial visualization
                            tx.send(PropagationEvent {
                                state: VisualizationState {
                                    current_wave: 0,
                                    nodes_reached: 1,
                                    max_time: 0.0,
                                    min_time: 0.0,
                                    avg_time: 0.0,
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

                                    let latency = network_sim.get_latency(connection_type);
                                    last_latency = latency;
                                    let arrival_time = event.arrival_time + (latency / 1000.0);
                                    
                                    let neighbor_node = Node::new(
                                        neighbor_index,
                                        arrival_time,
                                        event.node.depth + 1,
                                    );

                                    if !visited.contains(&neighbor_node.hash) {
                                        visited.insert(neighbor_node.hash);
                                        arrival_times.push(arrival_time);
                                        
                                        // Update max_depth when processing nodes
                                        if neighbor_node.depth > max_depth {
                                            max_depth = neighbor_node.depth;
                                        }

                                        event_queue.push(NetworkEvent {
                                            node: neighbor_node,
                                            arrival_time,
                                        });

                                        // Update visualization with progress
                                        if last_viz_update.elapsed() > Duration::from_millis(8) {
                                            let times: Vec<f64> = arrival_times.iter()
                                                .map(|&t| t * 1000.0)
                                                .collect();
                                            
                                            tx.send(PropagationEvent {
                                                state: VisualizationState {
                                                    current_wave: max_depth,
                                                    nodes_reached: visited.len(),
                                                    max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                                                    min_time: times.iter().fold(f64::INFINITY, |a: f64, &b| a.min(b)),
                                                    avg_time: times.iter().sum::<f64>() / times.len() as f64,
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

                            // Send final state with completion flag
                            let times: Vec<f64> = arrival_times.iter()
                                .map(|&t| t * 1000.0)
                                .collect();
                            
                            tx.send(PropagationEvent {
                                state: VisualizationState {
                                    current_wave: max_depth,
                                    nodes_reached: visited.len(),
                                    max_time: times.iter().fold(0.0_f64, |a: f64, &b| a.max(b)),
                                    min_time: times.iter().fold(f64::INFINITY, |a: f64, &b| a.min(b)),
                                    avg_time: times.iter().sum::<f64>() / times.len() as f64,
                                    last_latency,
                                    max_depth,
                                },
                                is_complete: true,
                            }).ok();
                        });
                    },
                    KeyCode::Char(' ') => paused = !paused,
                    KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc if paused => break Ok(()),
                    _ => {}
                }
            }
        }
    };

    // Cleanup: restore terminal state
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show,
    )?;
    crossterm::terminal::disable_raw_mode()?;

    result
}
