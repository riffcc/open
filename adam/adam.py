import pyglet
import pyglet.gl as gl
import random
import math
from pyglet.graphics import Batch
from pyglet.shapes import Circle, Line

# Thought class: represents a "thought" as a node in the network
class Thought:
    def __init__(self, id, value, x, y):
        self.id = id
        self.value = value
        self.connections = []
        self.x = x
        self.y = y
        self.vx = 0
        self.vy = 0

    def __str__(self):
        return f"Thought {self.id}: {self.value}"

class ThoughtNetwork:
    def __init__(self, num_thoughts=10):
        self.thoughts = []
        self.initialize_thoughts(num_thoughts)
        self.connect_thoughts()

    def initialize_thoughts(self, num_thoughts):
        values = ["Quantum", "Relativity", "Information", "Chaos", "Harmony"]
        for i in range(num_thoughts):
            x = random.uniform(50, 750)
            y = random.uniform(50, 550)
            thought = Thought(i, random.choice(values), x, y)
            self.thoughts.append(thought)

    def connect_thoughts(self, max_connections=3):
        for thought in self.thoughts:
            num_connections = random.randint(1, max_connections)
            for _ in range(num_connections):
                other_thought = random.choice(self.thoughts)
                if other_thought != thought and other_thought.id not in thought.connections:
                    thought.connections.append(other_thought.id)

    def evolve_thoughts(self):
        for thought in self.thoughts:
            if thought.connections:
                connected_values = [self.thoughts[conn_id].value for conn_id in thought.connections]
                thought.value = random.choice(connected_values)

    def update_positions(self):
        for thought in self.thoughts:
            # Apply forces
            for other in self.thoughts:
                if other != thought:
                    dx = other.x - thought.x
                    dy = other.y - thought.y
                    distance = math.sqrt(dx*dx + dy*dy)
                    if distance > 0:
                        force = (distance - 100) / 1000  # Adjust these values to change the network's behavior
                        thought.vx += force * dx / distance
                        thought.vy += force * dy / distance

            # Apply velocity
            thought.x += thought.vx
            thought.y += thought.vy

            # Damping
            thought.vx *= 0.9
            thought.vy *= 0.9

            # Keep within bounds
            thought.x = max(50, min(750, thought.x))
            thought.y = max(50, min(550, thought.y))

class NetworkVisualization(pyglet.window.Window):
    def __init__(self, network):
        super().__init__(800, 600, "Thought Network Visualization")
        self.network = network
        self.batch = Batch()
        self.nodes = []
        self.edges = []
        self.create_visuals()

    def create_visuals(self):
        for thought in self.network.thoughts:
            circle = Circle(thought.x, thought.y, 20, color=(100, 100, 255), batch=self.batch)
            self.nodes.append(circle)

        for thought in self.network.thoughts:
            for conn_id in thought.connections:
                other = self.network.thoughts[conn_id]
                line = Line(thought.x, thought.y, other.x, other.y, width=2, color=(200, 200, 200), batch=self.batch)
                self.edges.append(line)

    def update_visuals(self):
        for i, thought in enumerate(self.network.thoughts):
            self.nodes[i].x = thought.x
            self.nodes[i].y = thought.y

        edge_index = 0
        for thought in self.network.thoughts:
            for conn_id in thought.connections:
                other = self.network.thoughts[conn_id]
                self.edges[edge_index].x = thought.x
                self.edges[edge_index].y = thought.y
                self.edges[edge_index].x2 = other.x
                self.edges[edge_index].y2 = other.y
                edge_index += 1

    def on_draw(self):
        self.clear()
        self.batch.draw()

    def update(self, dt):
        self.network.update_positions()
        self.update_visuals()
        self.network.evolve_thoughts()

def main():
    network = ThoughtNetwork(num_thoughts=15)
    visualization = NetworkVisualization(network)
    pyglet.clock.schedule_interval(visualization.update, 1/60.0)
    pyglet.app.run()

if __name__ == "__main__":
    main()