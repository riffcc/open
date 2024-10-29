"""
Reality Simulation System
------------------------
A hexagonal network simulator with GPU acceleration and visualization.
"""

import numpy as np
from collections import defaultdict
import time
from tqdm import tqdm
import random
import math
import logging
from datetime import datetime
import json
from rich.console import Console
from rich.live import Live
from rich.table import Table
from rich.panel import Panel
import pytest
from numba import jit, cuda
import psutil

# =====================
# Core Network Classes
# =====================

class GlobalHexNetwork:
    """Hexagonal network with lazy computation."""
    
    def __init__(self, target_nodes=10_000_000):
        self.target_nodes = target_nodes
        self.layers = self.calculate_layers(target_nodes)
        self.node_count = self.calculate_total_nodes(self.layers)
        
        # Lazy data structures
        self._discovered_nodes = set()  # Only track nodes we've seen
        self._neighbor_cache = {}       # Cache neighbors as we find them
        self._layer_cache = {}          # Cache layer calculations
        
    @staticmethod
    def calculate_layers(target):
        """Calculate required layers for target node count."""
        nodes = 1
        layer = 0
        while nodes < target:
            layer += 1
            nodes += 6 * layer
        return layer

    @staticmethod
    def calculate_total_nodes(layers):
        """Calculate total nodes for given layers."""
        return 1 + sum(6 * layer for layer in range(1, layers + 1))

    def get_neighbors(self, node):
        """Lazily discover neighbors only when needed."""
        if node not in self._neighbor_cache:
            layer = self.get_node_layer(node)  # This gets cached
            
            # Only calculate neighbors when first accessed
            neighbors = []
            if self._should_explore_neighbors(node):
                neighbors.extend(self._get_local_neighbors(node, layer))
                neighbors.extend(self._get_regional_neighbors(node, layer))
                neighbors.extend(self._get_global_neighbors(node, layer))
            
            self._neighbor_cache[node] = neighbors
            self._discovered_nodes.add(node)
            
        return self._neighbor_cache[node]
    
    def _should_explore_neighbors(self, node):
        """Determine if we should explore this node's neighbors."""
        # Add your exploration heuristics here
        return True  # For now, explore everything we touch

    def get_node_layer(self, node):
        """Calculate which layer a node belongs to."""
        if node == 0:
            return 0
        
        total = 1
        layer = 1
        while total <= node:
            total += 6 * layer
            layer += 1
        return layer - 1

    def propagate_signal(self, start_node=0):
        """Propagate a signal through the network."""
        self.logger.log_milestone("\nStarting signal propagation from node 0...")
        
        visited = set([start_node])
        queue = [(start_node, 0)]  # (node, time)
        max_time = 0
        hops = defaultdict(int)
        
        with tqdm(total=self.node_count, desc="Propagating signal") as pbar:
            while queue:
                node, time = queue.pop(0)
                max_time = max(max_time, time)
                
                for neighbor, conn_type in self.get_neighbors(node):
                    if neighbor not in visited:
                        visited.add(neighbor)
                        new_time = time + self.latencies[conn_type]
                        queue.append((neighbor, new_time))
                        hops[len(visited)] += 1
                        pbar.update(1)
        
        return {
            'nodes_reached': len(visited),
            'max_time': max_time,
            'hops_histogram': dict(hops)
        }

    def simulate_attack(self, failure_rate):
        """Simulate network under attack with given node failure rate."""
        active_nodes = self.node_count - int(self.node_count * failure_rate)
        results = []
        
        for _ in range(3):  # Test 3 different starting points
            start_node = random.choice(self.nodes)
            self.logger.log_milestone(f"\nAttack Round {_+1}: Starting from node {start_node}")
            
            # Randomly disable nodes
            available_nodes = set(random.sample(self.nodes, active_nodes))
            
            # Test resilience
            visited = set([start_node]) if start_node in available_nodes else set()
            queue = [(start_node, 0)] if start_node in available_nodes else []
            max_time = 0
            
            with tqdm(total=active_nodes, desc="Testing resilience") as pbar:
                while queue:
                    node, time = queue.pop(0)
                    max_time = max(max_time, time)
                    
                    for neighbor, conn_type in self.get_neighbors(node):
                        if neighbor in available_nodes and neighbor not in visited:
                            visited.add(neighbor)
                            new_time = time + self.latencies[conn_type]
                            queue.append((neighbor, new_time))
                            pbar.update(1)
            
            coverage = len(visited) / active_nodes
            self.logger.log_milestone(f"Coverage achieved: {coverage*100:.2f}%")
            
            results.append({
                'nodes_reached': len(visited),
                'coverage': coverage,
                'max_time': max_time
            })
        
        return results

    def _get_local_neighbors(self, node, layer):
        """Get neighbors in the same layer."""
        if layer == 0:
            return []
        
        # Calculate position in layer
        nodes_before_layer = 1 + sum(6 * l for l in range(layer))
        position = node - nodes_before_layer
        nodes_in_layer = 6 * layer
        
        # Get neighbors (wrapping around the layer)
        neighbors = []
        # Connect to both adjacent nodes AND diagonal nodes in same layer
        for offset in [-1, 1, -2, 2]:  # Expanded connectivity
            neighbor_pos = (position + offset) % nodes_in_layer
            neighbor = nodes_before_layer + neighbor_pos
            if neighbor < self.node_count:
                neighbors.append(neighbor)
        
        return neighbors

    def _get_regional_neighbors(self, node, layer):
        """Get neighbors in adjacent layers."""
        neighbors = []
        
        # Inner layer connections (multiple connections)
        if layer > 0:
            inner_layer_size = 6 * (layer - 1)
            if inner_layer_size > 0:
                # Connect to multiple nodes in inner layer
                for offset in [-1, 0, 1]:
                    inner_node = node - (6 * layer) + offset
                    if 0 <= inner_node < self.node_count:
                        neighbors.append(inner_node)
        
        # Outer layer connections (multiple connections)
        if layer < self.layers - 1:
            outer_layer_size = 6 * (layer + 1)
            for offset in [-1, 0, 1]:
                outer_node = node + (6 * layer) + offset
                if outer_node < self.node_count:
                    neighbors.append(outer_node)
        
        return neighbors

    def _get_global_neighbors(self, node, layer):
        """Get long-distance neighbors (fractal connections)."""
        neighbors = []
        
        # Fractal connections: connect to nodes at 2^n layers away
        for n in range(1, int(math.log2(self.layers)) + 1):
            target_layer = layer + 2**n
            if target_layer < self.layers:
                # Connect to a node in the target layer
                nodes_before_target = 1 + sum(6 * l for l in range(target_layer))
                target_node = nodes_before_target + (node % (6 * target_layer))
                if target_node < self.node_count:
                    neighbors.append(target_node)
        
        return neighbors

# =====================
# Visualization System
# =====================
class NetworkVisualizer:
    def __init__(self, network):
        self.network = network
        self.console = Console()
        self.zoom_levels = {
            'macro':  {'chars': '⣿⣷⣯⣟⡿⢿⣻⣽⣾ ', 'density': 8},  # Braille for max density
            'medium': {'chars': '█▓▒░ ',           'density': 4},  # Blocks for medium view
            'micro':  {'chars': '⬡⬢·',             'density': 1}   # Hex for detailed view
        }
        self.current_zoom = 'medium'
        self.focus_point = None
        self.colors = {
            'active': 'green',
            'propagating': 'yellow',
            'failed': 'red'
        }

    def create_visualization(self, zoom=None):
        """Create multi-panel network visualization."""
        zoom = zoom or self.current_zoom
        layout = Table.grid()
        
        # Main view
        main_view = self._create_zoom_view(zoom, self.focus_point)
        layout.add_row(Panel(main_view, title=f"Network State [{zoom}]"))
        
        # Mini-map (always in macro)
        if zoom != 'macro':
            mini_map = self._create_zoom_view('macro')
            layout.add_row(Panel(mini_map, title="Overview"))
        
        # Stats panel
        stats = self._create_stats_panel()
        layout.add_row(Panel(stats, title="Network Statistics"))
        
        return layout

    def _create_zoom_view(self, zoom, focus=None):
        """Create visualization at specified zoom level."""
        table = Table(show_header=False, show_edge=False, pad=False)
        density = self.zoom_levels[zoom]['density']
        
        width = self.console.width
        height = int(width * 0.866)  # Maintain hex ratio
        
        for y in range(0, height, density):
            row = ""
            offset = " " * ((y//density) % 2)  # Hex grid offset
            for x in range(0, width, density):
                nodes = self._get_nodes_in_block(x, y, density)
                if nodes:
                    char = self._get_density_char(nodes, zoom)
                    color = self._get_block_color(nodes)
                    row += f"[{color}]{char}[/]"
                else:
                    row += " "
            table.add_row(offset + row)
        
        return table

    def _get_block_color(self, nodes):
        """Determine color based on node states."""
        states = [self.network.get_node_state(n) for n in nodes]
        if any(s['propagating'] for s in states):
            return self.colors['propagating']
        elif all(s['active'] for s in states):
            return self.colors['active']
        return self.colors['failed']

    def handle_input(self, key):
        """Handle interactive controls."""
        if key == '+':
            self._zoom_in()
        elif key == '-':
            self._zoom_out()
        elif key == 'f':
            self._toggle_focus()

# =====================
# Logging System
# =====================

class NetworkLogger:
    """Handles logging and metrics collection."""
    
    def __init__(self):
        self.start_time = datetime.now()
        self.log_file = f"network_reality_{self.start_time:%Y%m%d_%H%M%S}.log"
        
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s | %(message)s',
            handlers=[
                logging.FileHandler(self.log_file),
                logging.StreamHandler()
            ]
        )
        self.logger = logging.getLogger('RealitySimulation')
    
    def log_milestone(self, message, metrics=None):
        """Log important milestones and metrics."""
        self.logger.info(message)
        if metrics:
            self.logger.info(f"Metrics: {json.dumps(metrics, indent=2)}")
    
    def log_network_init(self, network):
        """Log network initialization details."""
        self.logger.info(f"Created network with {network.node_count:,} nodes in {network.layers:,} layers")

# =====================
# Testing Framework
# =====================

@pytest.fixture
def test_network():
    """Fixture for testing with smaller network."""
    return GlobalHexNetwork(target_nodes=1000)

def test_network_initialization(test_network):
    """Test network initialization."""
    assert test_network.node_count > 0
    assert test_network.layers > 0
    assert len(test_network.nodes) == test_network.node_count

def test_signal_propagation(test_network):
    """Test signal propagation."""
    result = test_network.propagate_signal(0)
    assert result['nodes_reached'] == test_network.node_count
    assert result['max_time'] > 0

def test_network_resilience(test_network):
    """Test network resilience under attack."""
    results = test_network.simulate_attack(0.3)  # 30% failure
    assert len(results) == 3
    assert all(r['coverage'] > 0 for r in results)

# =====================
# Visualization Tests
# =====================

def test_visualization_creation(test_network):
    """Test visualization creation at different zoom levels."""
    vis = NetworkVisualizer(test_network)
    
    for zoom in ['micro', 'medium', 'macro']:
        result = vis.create_visualization(zoom)
        assert result is not None
        assert isinstance(result, Table)

def test_visualization_colors(test_network):
    """Test node state color mapping."""
    vis = NetworkVisualizer(test_network)
    
    # Test active nodes
    nodes = [0, 1, 2]
    color = vis._get_block_color(nodes)
    assert color == vis.colors['active']
    
    # Test propagating nodes
    test_network.update_node_state(1, propagating=True)
    color = vis._get_block_color(nodes)
    assert color == vis.colors['propagating']

def test_visualization_zoom(test_network):
    """Test zoom level changes."""
    vis = NetworkVisualizer(test_network)
    
    # Test zoom in
    initial_density = vis.zoom_levels[vis.current_zoom]['density']
    vis._zoom_in()
    new_density = vis.zoom_levels[vis.current_zoom]['density']
    assert new_density < initial_density

def test_visualization_focus(test_network):
    """Test focus point tracking."""
    vis = NetworkVisualizer(test_network)
    
    # Test focus toggle
    vis._toggle_focus()
    assert vis.focus_point is not None
    
    vis._toggle_focus()
    assert vis.focus_point is None

# =====================
# Main Execution
# =====================

def main():
    """Main execution function."""
    network = GlobalHexNetwork()
    
    # Run reality verification
    network.logger.log_milestone("\nRunning reality verification tests...")
    result = network.propagate_signal()
    
    # Test network resilience
    network.logger.log_milestone("\nTesting network resilience...")
    failure_rates = [0.3, 0.31, 0.32, 0.33]  # 30%, 50%, 70%
    
    for rate in failure_rates:
        network.logger.log_milestone(f"\nSimulating {rate*100:.1f}% node failure...")
        results = network.simulate_attack(rate)
        
        network.logger.log_milestone(f"\nResults with {rate*100:.1f}% node failure:")
        for i, r in enumerate(results, 1):
            network.logger.log_milestone(f"Round {i}:")
            network.logger.log_milestone(f"  Nodes reached: {r['nodes_reached']:,}")
            network.logger.log_milestone(f"  Coverage: {r['coverage']*100:.2f}%")
            network.logger.log_milestone(f"  Max propagation time: {r['max_time']:.2f} seconds")

if __name__ == "__main__":
    main()
