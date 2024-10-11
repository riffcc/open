import networkx as nx
import matplotlib.pyplot as plt
import random
import time

# Initialize the thought network
thought_network = nx.Graph()

# Thought class: represents a "thought" as a node in the network
class Thought:
    def __init__(self, id, value):
        self.id = id
        self.value = value
        self.connections = []

    def __str__(self):
        return f"Thought {self.id}: {self.value}"

# Create an initial set of thoughts
def initialize_thoughts(num_thoughts):
    thoughts = []
    for i in range(num_thoughts):
        value = random.choice(["Quantum", "Relativity", "Information", "Chaos", "Harmony"])
        thought = Thought(i, value)
        thoughts.append(thought)
        thought_network.add_node(i, label=value)
    return thoughts

# Randomly connect thoughts in the network
def connect_thoughts(thoughts, max_connections=3):
    for thought in thoughts:
        num_connections = random.randint(1, max_connections)
        for _ in range(num_connections):
            other_thought = random.choice(thoughts)
            if other_thought != thought and other_thought.id not in thought.connections:
                thought.connections.append(other_thought.id)
                thought_network.add_edge(thought.id, other_thought.id)

# Simulate the evolution of thoughts (non-linear flow)
def evolve_thoughts(thoughts):
    for thought in thoughts:
        # Only evolve the thought if it has connected nodes
        if thought.connections:
            connected_values = [thoughts[conn_id].value for conn_id in thought.connections]
            thought.value = random.choice(connected_values)
            thought_network.nodes[thought.id]['label'] = thought.value
        else:
            # If no connections, maintain the current value
            thought_network.nodes[thought.id]['label'] = thought.value

# Visualize the thought network
def visualize_network():
    labels = nx.get_node_attributes(thought_network, 'label')
    pos = nx.spring_layout(thought_network)
    plt.figure(figsize=(8, 8))
    nx.draw(thought_network, pos, with_labels=True, labels=labels, node_size=1000, node_color="skyblue", font_size=10)
    plt.show()

# Main loop for creating and evolving thought network
def main(num_thoughts=10, iterations=10):
    thoughts = initialize_thoughts(num_thoughts)
    connect_thoughts(thoughts)
    visualize_network()

    for i in range(iterations):
        print(f"Iteration {i+1}")
        evolve_thoughts(thoughts)
        visualize_network()
        time.sleep(1)  # Pause between iterations to simulate real-time evolution

if __name__ == "__main__":
    main()
