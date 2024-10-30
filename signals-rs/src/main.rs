use std::collections::BinaryHeap;
use std::env;
use std::io;
use rand::Rng;
use std::cmp::Ordering;
use bitvec::prelude::*;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::{Block, Borders, canvas::{Canvas}},
    layout::Rect,
};
use crossterm::event::{self, Event, KeyCode};

#[derive(Debug)]
pub struct SimulationResult {
    pub nodes_reached: usize,
    pub fastest_time: f64,
    pub slowest_time: f64,
    pub average_time: f64,
    pub mode: String,
}

const HEX_DIRECTIONS: [(i32, i32); 6] = [
    (0, 1),    // Up
    (1, 0),    // Upper right
    (1, -1),   // Lower right
    (0, -1),   // Down
    (-1, 0),   // Lower left
    (-1, 1),   // Upper left
];

// For comparing floats in the priority queue
#[derive(Copy, Clone, PartialEq)]
struct OrderedFloat(f64);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct Node {
    q: i32,
    r: i32,
    arrival_time: OrderedFloat,
    depth: usize,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering so smallest time comes first
        other.arrival_time.cmp(&self.arrival_time)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Node {
    fn new(q: i32, r: i32, time: f64, depth: usize) -> Self {
        Self {
            q,
            r,
            arrival_time: OrderedFloat(time),
            depth,
        }
    }
}

#[derive(Clone)]
struct GlobalHexNetwork {
    node_count: usize,
}

impl GlobalHexNetwork {
    fn new(node_count: usize) -> Self {
        Self { node_count }
    }

    fn get_latency(&self, _connection_type: &str) -> f64 {
        let base_latency = 0.05;  // 50ms base latency
        let jitter = rand::thread_rng().gen_range(0.8..=1.2);
        base_latency * jitter
    }

    // Convert index in bitfield to hex coordinates
    fn index_to_hex(index: usize) -> (i32, i32) {
        // Using bit interleaving to maintain locality
        let mut q = 0i32;
        let mut r = 0i32;
        let mut i = index;
        
        for b in 0..16 {  // Support up to 32-bit coordinates
            q |= ((i & 1) as i32) << b;
            i >>= 1;
            r |= ((i & 1) as i32) << b;
            i >>= 1;
        }
        
        // Center the coordinates around 0,0
        let offset = 1 << 15;
        (q - offset, r - offset)
    }

    // Convert hex coordinates to bitfield index
    fn hex_to_index(&self, q: i32, r: i32) -> usize {
        // Add offset to make coordinates positive
        let offset = 1 << 15;
        let q = (q + offset) as u32;
        let r = (r + offset) as u32;
        
        // Interleave bits to maintain locality
        let mut index = 0usize;
        for b in 0..16 {
            index |= ((q >> b) & 1) as usize;
            index <<= 1;
            index |= ((r >> b) & 1) as usize;
            if b < 15 {
                index <<= 1;
            }
        }
        
        index % self.node_count
    }

    // Use this instead of hash_coordinates
    fn get_node_index(&self, q: i32, r: i32) -> usize {
        self.hex_to_index(q, r)
    }

    // Add routing table functionality
    fn get_route_to(&self, target_q: i32, target_r: i32) -> Vec<(i32, i32)> {
        let mut path = Vec::new();
        let mut current_q = 0;
        let mut current_r = 0;
        
        while current_q != target_q || current_r != target_r {
            // Find the direction that gets us closest to target
            let mut best_direction = (0, 0);
            let mut best_distance = i32::MAX;
            
            for (dq, dr) in HEX_DIRECTIONS.iter() {
                let next_q = current_q + dq;
                let next_r = current_r + dr;
                let distance = Self::hex_distance(next_q, next_r, target_q, target_r);
                
                if distance < best_distance {
                    best_distance = distance;
                    best_direction = (*dq, *dr);
                }
            }
            
            current_q += best_direction.0;
            current_r += best_direction.1;
            path.push((current_q, current_r));
        }
        
        path
    }

    fn hex_distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
        ((q1 - q2).abs() + (r1 - r2).abs() + (q1 + r1 - q2 - r2).abs()) / 2
    }

    fn reconstruct_path(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        came_from: std::collections::HashMap<(i32, i32), (i32, i32)>
    ) -> Vec<(i32, i32)> {
        let mut path = Vec::new();
        let mut current = to;
        
        while current != from {
            path.push(current);
            current = *came_from.get(&current).unwrap_or(&from);
        }
        
        path.reverse();
        path
    }

    fn get_active_connections(&self, q: i32, r: i32, origin_q: i32, origin_r: i32) -> Vec<(i32, i32)> {
        // Skip both "down" direction and any connection back to origin
        HEX_DIRECTIONS.iter()
            .enumerate()
            .filter(|(i, _)| *i != 3)  // Skip "down"
            .map(|(_, (dq, dr))| (q + dq, r + dr))
            .filter(|(new_q, new_r)| {
                let idx = self.hex_to_index(*new_q, *new_r);
                // Don't connect back to origin
                idx < self.node_count && (*new_q != origin_q || *new_r != origin_r)
            })
            .collect()
    }

    fn propagate_signal(&self) -> Result<(f64, f64, f64, usize), io::Error> {
        let mut visited = bitvec![0; self.node_count];
        let mut heap = BinaryHeap::new();
        let mut node_paths = vec![None; self.node_count];  // Store paths for all nodes
        
        let mut max_time: f64 = 0.0;
        let mut min_time: f64 = f64::MAX;
        let mut sum_time: f64 = 0.0;
        let mut nodes_reached: usize = 0;
        
        // Start from origin
        let origin_path = NodePath::new();
        heap.push(NetworkEvent::new(0, 0, 0.0, 0, origin_path));
        visited.set(0, true);
        
        while let Some(event) = heap.pop() {
            let current_idx = self.hex_to_index(event.q, event.r);
            if current_idx >= self.node_count {
                continue;
            }
            
            // Store the path for this node
            node_paths[current_idx] = Some(event.path.clone());
            
            if current_idx != 0 {
                max_time = max_time.max(event.arrival_time);
                min_time = min_time.min(event.arrival_time);
                sum_time += event.arrival_time;
                nodes_reached += 1;
            }

            // Try all six directions
            for (dir_idx, (dq, dr)) in HEX_DIRECTIONS.iter().enumerate() {
                let new_q = event.q + dq;
                let new_r = event.r + dr;
                let new_idx = self.hex_to_index(new_q, new_r);
                
                if new_idx < self.node_count 
                    && !visited[new_idx] 
                    && !(new_q == 0 && new_r == 0) {
                    
                    visited.set(new_idx, true);
                    
                    // Create new path by adding this step
                    let mut new_path = event.path.clone();
                    new_path.add_step(dir_idx);
                    
                    let arrival_time = event.arrival_time + self.get_latency("local");
                    heap.push(NetworkEvent::new(
                        new_q,
                        new_r,
                        arrival_time,
                        event.depth + 1,
                        new_path
                    ));
                }
            }
        }

        println!("\nPropagation Summary:");
        println!("Nodes reached: {}", nodes_reached);
        println!("Nodes missed: {}", self.node_count - nodes_reached - 1); // -1 for origin
        println!("Coverage: {:.1}%", (nodes_reached as f64 / self.node_count as f64) * 100.0);

        let avg_time = if nodes_reached > 0 { sum_time / nodes_reached as f64 } else { 0.0 };
        
        Ok((
            max_time * 1000.0,
            min_time * 1000.0,
            avg_time * 1000.0,
            nodes_reached
        ))
    }

    // Add method to find alternate routes when a path is blocked
    fn find_alternate_route(&self, from: (i32, i32), to: (i32, i32), blocked: &[(i32, i32)]) -> Option<Vec<(i32, i32)>> {
        let mut visited = bitvec![0; self.node_count];
        let mut heap = BinaryHeap::new();
        let mut came_from = std::collections::HashMap::new();
        
        heap.push(NetworkEvent::new(from.0, from.1, 0.0, 0));
        
        while let Some(event) = heap.pop() {
            let current = (event.q(), event.r());
            
            if current == to {
                // Reconstruct path
                let mut path = Vec::new();
                let mut current = current;
                while current != from {
                    path.push(current);
                    current = *came_from.get(&current)?;
                }
                path.reverse();
                return Some(path);
            }
            
            for (dq, dr) in HEX_DIRECTIONS.iter() {
                let next = (current.0 + dq, current.1 + dr);
                
                // Skip blocked nodes
                if blocked.contains(&next) {
                    continue;
                }
                
                let next_idx = self.hex_to_index(next.0, next.1);
                if next_idx >= self.node_count || visited[next_idx] {
                    continue;
                }
                
                visited.set(next_idx, true);
                came_from.insert(next, current);
                
                heap.push(NetworkEvent::new(
                    next.0,
                    next.1,
                    event.arrival_time + self.get_latency("local"),
                    event.depth() + 1
                ));
            }
        }
        
        None
    }

    fn draw(&self, f: &mut ratatui::Frame, area: Rect, nodes_reached: usize) {
        let canvas = Canvas::default()
            .block(Block::default().borders(Borders::ALL))
            .paint(|ctx| {
                // Draw origin node
                ctx.print(0.0, 0.0, "◉");

                let max_index = nodes_reached.min(self.node_count);
                let max_ring = (max_index as f64).sqrt() as i32;
                
                // Iterate through rings
                for ring in 0..=max_ring {
                    let positions = ring * 6; // Number of positions in this ring
                    if positions == 0 {
                        continue; // Skip center (already drawn)
                    }

                    // For each position in the ring
                    for i in 0..positions {
                        let angle = (i as f64) * 2.0 * std::f64::consts::PI / positions as f64;
                        let q = (ring as f64 * angle.cos()).round() as i32;
                        let r = (ring as f64 * angle.sin()).round() as i32;
                        
                        let idx = self.hex_to_index(q, r);
                        if idx >= max_index {
                            continue;
                        }

                        // Convert to screen coordinates
                        let screen_x = (q as f64 + r as f64 * 0.5) * 2.0;
                        let screen_y = r as f64 * 0.866 * 2.0;
                        
                        if screen_x.abs() > 20.0 || screen_y.abs() > 20.0 {
                            continue;
                        }

                        // Draw connections to neighbors
                        for (dq, dr) in HEX_DIRECTIONS.iter() {
                            let new_q = q + dq;
                            let new_r = r + dr;
                            let new_idx = self.hex_to_index(new_q, new_r);
                            if new_idx < max_index {
                                let new_x = (new_q as f64 + new_r as f64 * 0.5) * 2.0;
                                let new_y = new_r as f64 * 0.866 * 2.0;
                                ctx.print(
                                    (screen_x + new_x) / 2.0,
                                    (screen_y + new_y) / 2.0,
                                    "·"
                                );
                            }
                        }

                        // Draw the node
                        let depth = ring as usize % 6;
                        let symbol = match depth {
                            0 => "●",
                            1 => "◆",
                            2 => "■",
                            3 => "▲",
                            4 => "◈",
                            _ => "○",
                        };
                        ctx.print(screen_x, screen_y, symbol);
                    }
                }
            })
            .x_bounds([-20.0, 20.0])
            .y_bounds([-20.0, 20.0]);
        
        f.render_widget(canvas, area);
    }

    fn get_neighbors(&self, q: i32, r: i32) -> Vec<(i32, i32)> {
        HEX_DIRECTIONS.iter()
            .map(|(dq, dr)| (q + dq, r + dr))
            .filter(|(new_q, new_r)| {
                let idx = self.hex_to_index(*new_q, *new_r);
                idx < self.node_count
            })
            .collect()
    }

    fn find_route(&self, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        let mut visited = bitvec![0; self.node_count];
        let mut queue = std::collections::VecDeque::new();
        let mut came_from = std::collections::HashMap::new();
        
        queue.push_back(from);
        visited.set(self.hex_to_index(from.0, from.1), true);
        
        while let Some(current) = queue.pop_front() {
            if current == to {
                return Some(self.reconstruct_path(from, to, came_from));
            }
            
            for neighbor in self.get_neighbors(current.0, current.1) {
                let idx = self.hex_to_index(neighbor.0, neighbor.1);
                if !visited[idx] {
                    visited.set(idx, true);
                    came_from.insert(neighbor, current);
                    queue.push_back(neighbor);
                }
            }
        }
        
        None
    }
}

#[derive(Clone)]
struct NodePath {
    steps: Vec<usize>  // Indices into HEX_DIRECTIONS
}

impl NodePath {
    fn new() -> Self {
        NodePath { steps: Vec::new() }
    }

    fn add_step(&mut self, direction: usize) {
        self.steps.push(direction);
    }

    fn path_between(from: &NodePath, to: &NodePath) -> Vec<usize> {
        let mut path = Vec::new();
        for &step in from.steps.iter().rev() {
            path.push((step + 3) % 6);  // Backtrack to origin
        }
        path.extend(&to.steps);  // Forward to destination
        path
    }
}

struct NetworkEvent {
    q: i32,
    r: i32,
    arrival_time: f64,
    depth: usize,
    path: NodePath,  // Add path tracking to events
}

impl NetworkEvent {
    fn new(q: i32, r: i32, arrival_time: f64, depth: usize, path: NodePath) -> Self {
        Self { q, r, arrival_time, depth, path }
    }
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    let node_count = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10_000);
    
    let viz_mode = args.iter().any(|arg| arg == "--viz" || arg == "--vis");
    let network = GlobalHexNetwork::new(node_count);

    if viz_mode {
        // Visualization mode
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.draw(|f| {
            let area = f.area();
            network.draw(f, area, node_count);
        })?;

        loop {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen
        )?;
    } else {
        // Just run simulation and show results
        match network.propagate_signal() {
            Ok((max_time, min_time, avg_time, nodes_reached)) => {
                println!("Max time: {:.2}ms", max_time);
                println!("Min time: {:.2}ms", min_time);
                println!("Avg time: {:.2}ms", avg_time);
                println!("Nodes reached: {}", nodes_reached);
            }
            Err(e) => eprintln!("Error during signal propagation: {}", e),
        }
    }

    Ok(())
}
