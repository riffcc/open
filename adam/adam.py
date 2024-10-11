import asyncio
import websockets
import json
import random
import math
import sqlite3
import re
import threading
import time
from flask import Flask, render_template, send_from_directory
from flask_socketio import SocketIO

class Thought:
    def __init__(self, id, x, y, concept=""):
        self.id = id
        self.x = x
        self.y = y
        self.vx = 0
        self.vy = 0
        self.radius = 30
        self.color = f"rgb({random.randint(100, 255)}, {random.randint(100, 255)}, {random.randint(100, 255)})"
        self.connections = []
        self.concept = concept

    def to_dict(self):
        return {
            "id": self.id,
            "x": self.x,
            "y": self.y,
            "radius": self.radius,
            "color": self.color,
            "concept": self.concept,
            "connections": [c.id for c in self.connections]
        }

class ThoughtNetwork:
    def __init__(self, db_path='thoughts.db'):
        self.thoughts = []
        self.db_path = db_path
        self.init_db()
        self.load_thoughts()
        if not self.thoughts:
            self.generate_initial_thoughts()
        self.connect_thoughts()

    def init_db(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        c.execute('''CREATE TABLE IF NOT EXISTS thoughts
                     (id INTEGER PRIMARY KEY, concept TEXT, x REAL, y REAL, connections TEXT)''')
        conn.commit()
        conn.close()

    def load_thoughts(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        c.execute("SELECT * FROM thoughts")
        rows = c.fetchall()
        for row in rows:
            thought = Thought(row[0], row[2], row[3], row[1])
            self.thoughts.append(thought)
        conn.close()
        
        # Connect thoughts after all are loaded
        for row in rows:
            thought = next(t for t in self.thoughts if t.id == row[0])
            thought.connections = [next(t for t in self.thoughts if t.id == conn_id) 
                                   for conn_id in json.loads(row[4])]

    def save_thoughts(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        for thought in self.thoughts:
            c.execute("INSERT OR REPLACE INTO thoughts VALUES (?, ?, ?, ?, ?)",
                      (thought.id, thought.concept, thought.x, thought.y, 
                       json.dumps([t.id for t in thought.connections])))
        conn.commit()
        conn.close()

    def generate_initial_thoughts(self):
        for i in range(5):
            self.thoughts.append(Thought(i, random.uniform(50, 750), random.uniform(50, 550), ""))

    def connect_thoughts(self, max_connections=2):
        for thought in self.thoughts:
            num_connections = random.randint(0, max_connections)
            thought.connections = random.sample([t for t in self.thoughts if t != thought], num_connections)

    def update_positions(self):
        for thought in self.thoughts:
            for other in thought.connections:
                dx = other.x - thought.x
                dy = other.y - thought.y
                distance = math.sqrt(dx*dx + dy*dy)
                if distance > 0:
                    force = (distance - 100) / 500
                    thought.vx += force * dx / distance
                    thought.vy += force * dy / distance
            
            thought.vx *= 0.95
            thought.vy *= 0.95
            thought.x += thought.vx
            thought.y += thought.vy
            thought.x = max(thought.radius, min(750 - thought.radius, thought.x))
            thought.y = max(thought.radius, min(550 - thought.radius, thought.y))

    def add_thought(self, x, y, concept=""):
        new_thought = Thought(len(self.thoughts), x, y, concept)
        self.thoughts.append(new_thought)
        self.save_thoughts()
        return new_thought

    def process_input(self, user_input):
        words = re.findall(r'\w+', user_input.lower())
        new_thoughts = []
        for word in words:
            new_thought = self.add_thought(random.uniform(50, 750), random.uniform(50, 550), word)
            new_thoughts.append(new_thought)
        
        for i in range(len(new_thoughts) - 1):
            new_thoughts[i].connections.append(new_thoughts[i+1])
            new_thoughts[i+1].connections.append(new_thoughts[i])
        
        self.save_thoughts()
        return f"Processed input and added {len(new_thoughts)} new thoughts."

    def generate_response(self):
        if len(self.thoughts) < 3:
            return "I need more information to generate a response."
        
        selected_thoughts = random.sample(self.thoughts, 3)
        response = " ".join([t.concept for t in selected_thoughts])
        return response.capitalize() + "."

app = Flask(__name__)
socketio = SocketIO(app, cors_allowed_origins="*")
network = ThoughtNetwork()

@app.route('/')
def index():
    return render_template('index.html')

@socketio.on('connect')
def handle_connect():
    print('Client connected')
    emit_network_state()

def emit_network_state():
    network_state = {
        "thoughts": [thought.to_dict() for thought in network.thoughts]
    }
    socketio.emit('network_update', network_state)

def update_network():
    while True:
        network.update_positions()
        emit_network_state()
        time.sleep(0.05)

def terminal_interface():
    print("Welcome to the Thought Network Terminal!")
    print("Type your input to teach the network. Type 'exit' to quit.")
    
    while True:
        user_input = input("> ")
        if user_input.lower() == 'exit':
            break
        
        print(network.process_input(user_input))
        time.sleep(1)
        print("Network's response:", network.generate_response())
        emit_network_state()

if __name__ == '__main__':
    update_thread = threading.Thread(target=update_network)
    update_thread.daemon = True
    update_thread.start()

    terminal_thread = threading.Thread(target=terminal_interface)
    terminal_thread.daemon = True
    terminal_thread.start()

    socketio.run(app, debug=True, use_reloader=False)
