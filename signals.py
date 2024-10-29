import numpy as np
from collections import defaultdict
import time
from tqdm import tqdm
import random  # Using standard random
import math  # Add this import

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
        hops_histogram = defaultdict(int)
        
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
                hops_histogram[len(visited)] += 1
                
                neighbors = self.get_neighbors(current)
                for neighbor, distance_type in neighbors:
                    if neighbor not in visited:
                        latency = self.latencies[distance_type]
                        queue.append((neighbor, t + latency))
        
        return {
            'max_time': max_time,
            'nodes_reached': nodes_reached,
            'hops_histogram': dict(hops_histogram)
        }

    def get_neighbors(self, node):
        # Simpler, faster neighbor calculation
        neighbors = []
        
        # Direct neighbors (always connect to adjacent nodes)
        local_neighbors = [
            node + 1,
            node - 1
        ]
        
        # Layer jumps (connect to nodes ~1000 away)
        jump_size = 1000
        regional_neighbors = [
            node + jump_size,
            node - jump_size
        ]
        
        # Add local neighbors
        for n in local_neighbors:
            if 0 <= n < self.node_count:
                neighbors.append((n, 'local'))
                
        # Add regional neighbors
        for n in regional_neighbors:
            if 0 <= n < self.node_count:
                neighbors.append((n, 'regional'))
        
        return neighbors

    def get_node_layer(self, node):
        # Quick layer calculation
        if node == 0:
            return 0
        layer = int((3 + math.sqrt(9 + 12 * node)) / 6)
        return layer

    def simulate_attack(self, failure_rate=0.5):
        """Simulate network under attack by disabling nodes"""
        print(f"\nSimulating {failure_rate*100}% node failure...")
        
        # Randomly select nodes to fail
        total_failures = int(self.node_count * failure_rate)
        failed_nodes = set(random.sample(self.nodes, total_failures))
        
        # Run propagation with node failures
        results = []
        for attack_round in range(3):  # Test multiple patterns
            start_node = random.choice([n for n in self.nodes if n not in failed_nodes])
            
            print(f"\nAttack Round {attack_round + 1}: Starting from node {start_node}")
            result = self.propagate_signal_under_attack(start_node, failed_nodes)
            results.append(result)
            
            coverage = result['nodes_reached'] / (self.node_count - len(failed_nodes))
            print(f"Coverage achieved: {coverage*100:.2f}%")
        
        return results

    def propagate_signal_under_attack(self, start_node, failed_nodes):
        visited = set()
        times = defaultdict(float)
        queue = [(start_node, 0)]
        
        with tqdm(total=self.node_count - len(failed_nodes), 
                 desc="Testing resilience") as pbar:
            while queue:
                current, t = queue.pop(0)
                if current in visited or current in failed_nodes:
                    continue
                
                visited.add(current)
                pbar.update(1)
                times[current] = t
                
                neighbors = self.get_neighbors(current)
                for neighbor, distance_type in neighbors:
                    if (neighbor not in visited and 
                        neighbor not in failed_nodes):
                        latency = self.latencies[distance_type]
                        queue.append((neighbor, t + latency))
        
        return {
            'max_time': max(times.values()) if times else 0,
            'nodes_reached': len(visited),
            'coverage': len(visited) / (self.node_count - len(failed_nodes))
        }

def get_percentile(data, p):
    """Calculate percentile from a list of values"""
    sorted_data = sorted(data)
    k = (len(sorted_data) - 1) * (p/100.0)
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return sorted_data[int(k)]
    d0 = sorted_data[int(f)] * (c-k)
    d1 = sorted_data[int(c)] * (k-f)
    return d0 + d1

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

# Add histogram summary
print("\nHop distribution summary:")
hops = results['hops_histogram']
percentiles = [50, 90, 95, 99]
for p in percentiles:
    hop_count = get_percentile(list(hops.keys()), p)
    print(f"{p}th percentile hops: {hop_count:.0f}")

print("\nTesting network resilience...")
failure_rates = [0.3, 0.5, 0.7]  # Test different failure rates
for rate in failure_rates:
    results = network.simulate_attack(rate)
    print(f"\nResults with {rate*100}% node failure:")
    for i, r in enumerate(results):
        print(f"Round {i+1}:")
        print(f"  Nodes reached: {r['nodes_reached']:,}")
        print(f"  Coverage: {r['coverage']*100:.2f}%")
        print(f"  Max propagation time: {r['max_time']/1000:.2f} seconds")

# Add these precise test points
failure_rates = [
    0.499,    # 49.9%
    0.4999,   # 49.99%
    0.49999,  # 49.999%
    0.499999, # 49.9999%
    0.5,      # 50% (our known breakdown point)
]

print("\nTesting precise failure thresholds...")
for rate in failure_rates:
    print(f"\nSimulating {rate*100:.4f}% node failure...")
    results = network.simulate_attack(rate)
    print(f"\nResults with {rate*100:.4f}% node failure:")
    for i, r in enumerate(results):
        print(f"Round {i+1}:")
        print(f"  Nodes reached: {r['nodes_reached']:,}")
        print(f"  Coverage: {r['coverage']*100:.4f}%")
        print(f"  Max propagation time: {r['max_time']/1000:.2f} seconds")
