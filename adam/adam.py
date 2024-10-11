import pyglet
from pyglet.window import mouse, key
import random
import math
import sqlite3
import json
import requests
import threading
import time

class Thought:
    def __init__(self, id, x, y, concept="", details=""):
        self.id = id
        self.x = x
        self.y = y
        self.vx = 0
        self.vy = 0
        self.radius = random.uniform(10, 20)
        self.color = (random.randint(100, 255), random.randint(100, 255), random.randint(100, 255))
        self.connections = []
        self.concept = concept
        self.details = details

class ThoughtNetwork:
    def __init__(self, db_path='thoughts.db'):
        # ... (keep the existing initialization code)

    def init_db(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        c.execute('''CREATE TABLE IF NOT EXISTS thoughts
                     (id INTEGER PRIMARY KEY, concept TEXT, details TEXT, x REAL, y REAL, connections TEXT)''')
        conn.commit()
        conn.close()

    def load_thoughts(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        c.execute("SELECT * FROM thoughts")
        rows = c.fetchall()
        for row in rows:
            thought = Thought(row[0], row[3], row[4], row[1], row[2])
            thought.connections = json.loads(row[5])
            self.thoughts.append(thought)
        conn.close()

    def save_thoughts(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        for thought in self.thoughts:
            c.execute("INSERT OR REPLACE INTO thoughts VALUES (?, ?, ?, ?, ?, ?)",
                      (thought.id, thought.concept, thought.details, thought.x, thought.y, 
                       json.dumps([t.id for t in thought.connections])))
        conn.commit()
        conn.close()

    def learn(self):
        thought = random.choice(self.thoughts)
        question = f"Explain the concept of {thought.concept} in one sentence."
        response = self.ask_ollama(question)
        
        new_thought = Thought(len(self.thoughts), random.uniform(50, 750), random.uniform(50, 550), 
                              response.split()[0], response)  # Use first word as concept, full sentence as details
        
        new_thought.connections.append(thought)
        thought.connections.append(new_thought)
        self.thoughts.append(new_thought)
        self.save_thoughts()

    # ... (keep other methods the same)

class NetworkVisualization(pyglet.window.Window):
    def __init__(self, network):
        super().__init__(1024, 768, "Dynamic Thought Network with Concept Visualization")
        self.network = network
        self.batch = pyglet.graphics.Batch()
        self.circles = []
        self.labels = []
        self.lines = []
        self.details_label = pyglet.text.Label('', x=10, y=10, width=300, multiline=True, 
                                               font_size=10, color=(255, 255, 255, 255))
        self.selected_thought = None
        self.update_visuals()

    def update_visuals(self):
        self.batch = pyglet.graphics.Batch()
        self.circles = [pyglet.shapes.Circle(t.x, t.y, t.radius, color=t.color, batch=self.batch) 
                        for t in self.network.thoughts]
        self.labels = [pyglet.text.Label(t.concept, x=t.x, y=t.y, anchor_x='center', anchor_y='center', 
                                         font_size=8, color=(0, 0, 0, 255), batch=self.batch) 
                       for t in self.network.thoughts]
        self.lines = []
        for t in self.network.thoughts:
            for c in t.connections:
                self.lines.append(pyglet.shapes.Line(t.x, t.y, c.x, c.y, width=1, color=(200, 200, 200), batch=self.batch))

    def on_draw(self):
        self.clear()
        self.batch.draw()
        if self.selected_thought:
            self.details_label.text = f"Concept: {self.selected_thought.concept}\nDetails: {self.selected_thought.details}"
            self.details_label.draw()

    def update(self, dt):
        self.network.update_positions()
        self.update_visuals()

    def on_mouse_press(self, x, y, button, modifiers):
        if button == mouse.LEFT:
            self.select_thought(x, y)
        elif button == mouse.RIGHT:
            threading.Thread(target=self.network.learn).start()

    def select_thought(self, x, y):
        for thought in self.network.thoughts:
            if math.sqrt((thought.x - x)**2 + (thought.y - y)**2) < thought.radius:
                self.selected_thought = thought
                break
        else:
            self.selected_thought = None

    def on_key_press(self, symbol, modifiers):
        if symbol == key.SPACE:
            self.network.learn()

def main():
    network = ThoughtNetwork()
    visualization = NetworkVisualization(network)
    pyglet.clock.schedule_interval(visualization.update, 1/60.0)
    pyglet.app.run()

if __name__ == "__main__":
    main()
