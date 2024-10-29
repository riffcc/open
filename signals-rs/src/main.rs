use std::collections::{HashMap, VecDeque};
use std::f64;
use std::time::Instant;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use ratatui::layout::{Layout, Constraint, Direction};
use ratatui::style::{Style, Color};
use crossterm::event::{self, Event, KeyCode};
use std::io;

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

fn main() -> Result<(), io::Error> {
    let mut network = GlobalHexNetwork::new(10_000); // Smaller node count for testing
    let mut failure_rate = 0.0;
    let mut attack_results = Vec::new();

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
                .split(f.size());

            let block = Block::default().title("GlobalHexNetwork Simulation").borders(Borders::ALL);
            f.render_widget(block, chunks[0]);

            let (max_time, nodes_reached) = network.propagate_signal(0);
            let progress = (nodes_reached as f64 / network.node_count as f64) * 100.0;

            let gauge = Gauge::default()
                .block(Block::default().title("Propagation Progress").borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(progress as u16);
            f.render_widget(gauge, chunks[1]);

            let items: Vec<ListItem> = attack_results
                .iter()
                .enumerate()
                .map(|(i, (time, reached))| {
                    ListItem::new(format!(
                        "Round {}: Reached {} nodes, Max time: {:.2} ms",
                        i + 1,
                        reached,
                        time
                    ))
                })
                .collect();
            let list = List::new(items)
                .block(Block::default().title("Attack Results").borders(Borders::ALL));
            f.render_widget(list, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('+') => {
                        failure_rate += 0.1;
                        if failure_rate > 1.0 {
                            failure_rate = 1.0;
                        }
                        attack_results = network.simulate_attack(failure_rate);
                    }
                    KeyCode::Char('-') => {
                        failure_rate -= 0.1;
                        if failure_rate < 0.0 {
                            failure_rate = 0.0;
                        }
                        attack_results = network.simulate_attack(failure_rate);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
