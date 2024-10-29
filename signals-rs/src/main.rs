use std::collections::{HashMap, HashSet, BinaryHeap};
use std::cmp::Ordering;
use std::env;
use std::io;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::layout::{Layout, Constraint, Direction};
use ratatui::style::{Style, Color};
use crossterm::event::{self, Event, KeyCode};

#[derive(Copy, Clone)]
struct Node {
    index: usize,
    time: f64,
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
    layers: usize,
    latencies: HashMap<&'static str, f64>,
}

impl GlobalHexNetwork {
    fn new(target_nodes: usize) -> Self {
        let layers = Self::calculate_layers(target_nodes);
        let node_count = Self::calculate_total_nodes(layers);
        let mut latencies = HashMap::new();
        latencies.insert("local", 5.0);
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
        let mut visited = HashSet::new();
        let mut heap = BinaryHeap::new();
        heap.push(Node { index: start_node, time: 0.0 });
        
        let mut max_time: f64 = 0.0;
        let mut nodes_reached = 0;
        
        while let Some(Node { index, time }) = heap.pop() {
            if visited.contains(&index) {
                continue;
            }
            visited.insert(index);
            nodes_reached += 1;
            max_time = max_time.max(time);

            for &(neighbor, latency) in &self.lazy_get_neighbors(index) {
                if !visited.contains(&neighbor) {
                    heap.push(Node { index: neighbor, time: time + latency });
                }
            }
        }
        (max_time, nodes_reached)
    }

    fn lazy_get_neighbors(&self, node: usize) -> Vec<(usize, f64)> {
        let mut neighbors = Vec::new();
        if node < self.node_count - 1 {
            neighbors.push((node + 1, *self.latencies.get("local").unwrap()));
        }
        if node > 0 {
            neighbors.push((node - 1, *self.latencies.get("local").unwrap()));
        }
        if node + 1000 < self.node_count {
            neighbors.push((node + 1000, *self.latencies.get("regional").unwrap()));
        }
        if node >= 1000 {
            neighbors.push((node - 1000, *self.latencies.get("regional").unwrap()));
        }
        neighbors
    }
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    let node_count = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10_000);

    let network = GlobalHexNetwork::new(node_count);
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let (max_time, nodes_reached) = network.propagate_signal(0);
    let progress = (nodes_reached as f64 / network.node_count as f64) * 100.0;

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

            let block = Block::default().title("GlobalHexNetwork Simulation").borders(Borders::ALL);
            f.render_widget(block, chunks[0]);

            let gauge = Gauge::default()
                .block(Block::default().title("Propagation Progress").borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(progress as u16);
            f.render_widget(gauge, chunks[1]);

            let stats = Paragraph::new(format!(
                "Nodes Reached: {}\nMax Time: {:.2} ms",
                nodes_reached, max_time
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
    println!("Nodes Reached: {}", nodes_reached);
    println!("Max Time: {:.2} ms", max_time);

    Ok(())
}
