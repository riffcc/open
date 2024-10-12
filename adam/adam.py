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
import requests
import os
import logging

# Configure logging to a file
logging.basicConfig(filename='thought_network_interactions.log', level=logging.INFO, 
                    format='%(asctime)s - %(message)s')

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
        self.ollama_url = os.environ.get('OLLAMA_URL', "http://localhost:11434/api/generate")
        self.ollama_model = "gemma2"
        self.continuous_learning = False
        self.continuous_learning_thread = None

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
        initial_concepts = ["hello", "world", "thinking", "learning", "growing"]
        for i, concept in enumerate(initial_concepts):
            self.thoughts.append(Thought(i, random.uniform(50, 750), random.uniform(50, 550), concept))

    def connect_thoughts(self, max_connections=3):
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
                    force = (distance - 100) / 1000  # Reduced force for more stability
                    thought.vx += force * dx / distance
                    thought.vy += force * dy / distance
            
            thought.vx *= 0.9  # Increased damping for stability
            thought.vy *= 0.9
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
        
        response_length = random.randint(1, 5)  # Variable response length
        selected_thoughts = random.sample(self.thoughts, response_length)
        response = " ".join([t.concept for t in selected_thoughts])
        return response.capitalize() + "."

    def ask_ollama(self, prompt):
        data = {
            "model": self.ollama_model,
            "prompt": prompt
        }
        try:
            response = requests.post(self.ollama_url, json=data, stream=True, timeout=10)
            if response.status_code == 200:
                full_response = ""
                for line in response.iter_lines():
                    if line:
                        json_response = json.loads(line)
                        full_response += json_response.get('response', '')
                        if json_response.get('done', False):
                            break
                return full_response
            else:
                print(f"Error from Ollama: {response.status_code}")
                print(response.text)
                return f"I'm thinking about that a little more. Let's try again soon!"
        except requests.exceptions.RequestException as e:
            return f"I'm having a little trouble connecting right now, but we'll figure it out together! (Error: {str(e)})"
        except json.JSONDecodeError as e:
            print(f"JSON decode error: {e}")
            return "I'm having trouble understanding the response. Let's try again!"

    def interact_with_ollama(self):
        prompt = self.generate_response()
        logging.info(f"Network asks Ollama: {prompt}")
        ollama_response = self.ask_ollama(f"You are talking to a simple thought network that is learning language. It said: '{prompt}'. Please respond in a way that might help it learn language, counting, or the alphabet. Keep your response simple and gentle.")
        logging.info(f"Ollama responds: {ollama_response}")
        
        # Process the Ollama response and add it to the network
        self.process_input(ollama_response)
        
        return ollama_response

    def continuous_learning_loop(self):
        while self.continuous_learning:
            try:
                network_thought = self.generate_response()
                print(f"Network: {network_thought}")
                
                ollama_response = self.interact_with_ollama()
                print(f"Ollama: {ollama_response}")
                
                network_response = self.generate_response()
                print(f"Network response: {network_response}")
                
                time.sleep(5)  # Reduced to 5 seconds
            except Exception as e:
                logging.error(f"Error in continuous learning loop: {e}")
                print(f"Error in continuous learning: {e}")
                time.sleep(10)  # Reduced to 10 seconds for errors

    def toggle_continuous_learning(self):
        self.continuous_learning = not self.continuous_learning
        if self.continuous_learning:
            if self.continuous_learning_thread is None or not self.continuous_learning_thread.is_alive():
                self.continuous_learning_thread = threading.Thread(target=self.continuous_learning_loop)
                self.continuous_learning_thread.daemon = True
                self.continuous_learning_thread.start()
            return "Continuous learning enabled."
        else:
            return "Continuous learning disabled."

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
        time.sleep(0.1)

def terminal_interface():
    print("Welcome to the Thought Network Terminal!")
    print("Type your input to interact with the network.")
    print("Press Enter with no input to let the network think on its own.")
    print("Press 'Home' key to interact with Ollama.")
    print("Type 'toggle' to enable/disable continuous learning.")
    print("Type 'exit' to quit.")
    
    while True:
        user_input = input("> ")
        if user_input.lower() == 'exit':
            break
        elif user_input.lower() == 'toggle':
            result = network.toggle_continuous_learning()
            print(result)
        elif user_input == "":
            print("Network's thought:", network.generate_response())
        elif user_input == "\x1b[H":  # Home key
            network_thought = network.generate_response()
            print("Network asks Ollama:", network_thought)
            ollama_response = network.interact_with_ollama()
            print("Ollama responds:", ollama_response)
            print("Network's response:", network.generate_response())
        else:
            print(network.process_input(user_input))
            print("Network's response:", network.generate_response())
        emit_network_state()

if __name__ == '__main__':
    network = ThoughtNetwork()

    # Start the background network update thread
    update_thread = threading.Thread(target=update_network)
    update_thread.daemon = True
    update_thread.start()

    # Start the terminal interface in a separate thread
    terminal_thread = threading.Thread(target=terminal_interface)
    terminal_thread.daemon = True
    terminal_thread.start()

    # Start the Flask application in the main thread
    socketio.run(app, debug=False, use_reloader=False, port=5000)
