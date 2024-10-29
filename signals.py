import numpy as np
from collections import defaultdict
import time
from tqdm import tqdm
import random  # Using standard random

class GlobalHexNetwork:
    def __init__(self, target_nodes=10_000_000):  # 10 million
        print("Calculating network size...")
        self.layers = self.calculate_layers(target_nodes)
        self.node_count = self.calculate_total_nodes(self.layers)
        self.nodes = list(range(self.node_count))
        
        print(f"Initializing latencies for {self.node_count:,} nodes...")
        self.latencies = {
            'local': random.gauss(5, 1),      # 5ms ± 1ms local
            'regional': random.gauss(25, 5),   # 25ms ± 5ms regional
            'global': random.gauss(100, 20),
        }
    
    def calculate_layers(self, target):
        # Each layer N adds 6N new nodes
        # Total nodes = 1 + 6(1 + 2 + ... + N)
        nodes = 1
        layer = 0
        while nodes < target:
            layer += 1
            nodes += 6 * layer
        return layer
    
    def calculate_total_nodes(self, layers):
        # Calculate total nodes for given layers
        total = 1  # Center node
        for layer in range(1, layers + 1):
            total += 6 * layer
        return total
    
    def propagate_signal(self, start_node=0):
        print(f"\nStarting signal propagation from node {start_node}...")
        visited = set()
        times = defaultdict(float)
        queue = [(start_node, 0)]
        
        max_time = 0
        nodes_reached = 0
        
        with tqdm(total=self.node_count, desc="Propagating signal") as pbar:
            while queue and nodes_reached < self.node_count:
                current, t = queue.pop(0)
                if current in visited:
                    continue
                
                visited.add(current)
                nodes_reached += 1
                pbar.update(1)
                
                times[current] = t
                max_time = max(max_time, t)
                
                neighbors = self.get_neighbors(current)
                for neighbor, distance_type in neighbors:
                    if neighbor not in visited:
                        latency = self.latencies[distance_type]
                        queue.append((neighbor, t + latency))
        
        return {
            'max_time': max_time,
            'nodes_reached': nodes_reached
        }

    def get_neighbors(self, node):
        # Return list of (neighbor, distance_type) based on hexagonal geometry
        # Simplified for demonstration
        neighbors = []
        for i in range(6):  # 6 neighbors in hexagonal grid
            neighbor = node + i + 1
            if neighbor >= len(self.nodes):
                continue
            
            # Determine distance type based on node positions
            if i < 2:
                dist_type = 'local'
            elif i < 4:
                dist_type = 'regional'
            else:
                dist_type = 'global'
                
            neighbors.append((neighbor, dist_type))
        return neighbors

# Run simulation
print("Initializing global network simulation...")
network = GlobalHexNetwork()
print(f"Created network with {network.node_count:,} nodes in {network.layers:,} layers")

print("\nSimulating signal propagation...")
start_time = time.time()
results = network.propagate_signal()
end_time = time.time()

print(f"\nResults:")
print(f"Maximum propagation time: {results['max_time']/1000:.2f} seconds")
print(f"Nodes reached: {results['nodes_reached']:,}")
print(f"Simulation took: {end_time - start_time:.2f} seconds")

# Print hop distribution summary
hops = results['hops_histogram']
print("\nHop distribution summary:")
percentiles = [50, 90, 95, 99]
for p in percentiles:
    hop_count = np.percentile(list(hops.keys()), p)
    print(f"{p}th percentile hops: {hop_count:.0f}")
