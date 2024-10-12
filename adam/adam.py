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
from flask_socketio import SocketIO, emit
import requests
import os
import logging
import networkx as nx
import matplotlib.pyplot as plt
from collections import Counter

# Configure logging to a file
logging.basicConfig(filename='thought_network_interactions.log', level=logging.INFO, 
                    format='%(asctime)s - %(message)s')

class Thought:
    def __init__(self, id, concept="", trust=0.5):
        self.id = id
        self.concept = concept
        self.trust = trust
        self.connections = {}  # Dictionary of connected Thoughts and their connection strengths
        self.occurrences = 1  # Number of times this concept has been encountered
        self.metadata = {}  # Additional metadata for the thought

    def to_dict(self):
        return {
            "id": self.id,
            "concept": self.concept,
            "trust": self.trust,
            "connections": {c.id: strength for c, strength in self.connections.items()},
            "occurrences": self.occurrences,
            "metadata": self.metadata
        }

    def update_trust(self, change, feedback_source):
        self.trust += change
        # Ensure trust is between 0.0 and 1.0
        self.trust = max(0.0, min(self.trust, 1.0))
        self.last_activity = time.time()

    def decay_trust(self, decay_rate=0.01, decay_threshold=86400):
        """Decay trust over time if node has been inactive."""
        if time.time() - self.last_activity > decay_threshold:
            self.trust = max(0.0, self.trust - decay_rate)
            self.last_activity = time.time()


class ThoughtNetwork:
    def __init__(self, db_path='thoughts.db'):
        self.thoughts = {}
        self.db_path = db_path
        self.init_db()
        self.load_thoughts()
        if not self.thoughts:
            self.generate_initial_thoughts()
        self.ollama_url = os.environ.get('OLLAMA_URL', "http://localhost:11434/api/generate")
        self.ollama_model = "gemma2"
        self.continuous_learning = False
        self.continuous_learning_thread = None
        self.high_trust_thoughts = set()
        self.thought_counter = Counter()

    def init_db(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        c.execute('''CREATE TABLE IF NOT EXISTS thoughts
                     (id INTEGER PRIMARY KEY, concept TEXT, trust REAL)''')
        c.execute('''CREATE TABLE IF NOT EXISTS connections
                     (thought_id INTEGER, connected_id INTEGER, strength REAL,
                      PRIMARY KEY (thought_id, connected_id))''')
        conn.commit()
        conn.close()

    def load_thoughts(self):
        connection = sqlite3.connect(self.db_path)
        c = connection.cursor()
        c.execute("SELECT * FROM thoughts")
        rows = c.fetchall()
        for row in rows:
            thought = Thought(row[0], row[1], row[2])
            self.thoughts[thought.id] = thought
        
        c.execute("SELECT * FROM connections")
        connections = c.fetchall()
        for conn in connections:
            thought_id, connected_id, strength = conn
            if thought_id in self.thoughts and connected_id in self.thoughts:
                self.thoughts[thought_id].connections[self.thoughts[connected_id]] = strength
        connection.close()

    def save_thoughts(self):
        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        for thought in self.thoughts.values():
            c.execute("INSERT OR REPLACE INTO thoughts VALUES (?, ?, ?)",
                      (thought.id, thought.concept, thought.trust))
            for connected, strength in thought.connections.items():
                c.execute("INSERT OR REPLACE INTO connections VALUES (?, ?, ?)",
                          (thought.id, connected.id, strength))
        conn.commit()
        conn.close()

    def generate_initial_thoughts(self):
        initial_concepts = ["hello", "world", "thinking", "learning", "growing"]
        for i, concept in enumerate(initial_concepts):
            self.add_thought(concept)

    def add_thought(self, concept, trust=0.5, occurrences=1, metadata=None):
        new_id = max(self.thoughts.keys(), default=-1) + 1
        new_thought = Thought(new_id, concept, trust)
        new_thought.occurrences = occurrences
        if metadata:
            new_thought.metadata = metadata
        self.thoughts[new_id] = new_thought
        self.save_thoughts()
        return new_thought

    def connect_thoughts(self, thought1, thought2, strength=0.5):
        thought1.connections[thought2] = strength
        thought2.connections[thought1] = strength
        self.save_thoughts()

    def reinforce_thought(self, thought, amount=0.1):
        thought.update_trust(amount, "system")
        self.save_thoughts()
        if thought.trust > 0.7:
            self.high_trust_thoughts.add(thought.id)
        self.thought_counter[thought.id] += 1

    def weaken_thought(self, thought, amount=0.1):
        thought.update_trust(-amount, "system")
        self.save_thoughts()

    def process_input(self, user_input):
        words = user_input.lower().split()
        new_thoughts = []
        for word in words:
            existing_thought = next((t for t in self.thoughts.values() if t.concept == word), None)
            if existing_thought:
                self.reinforce_thought(existing_thought)
            else:
                new_thought = self.add_thought(word)
                new_thoughts.append(new_thought)

        for i in range(len(new_thoughts)):
            if i > 0:
                self.connect_thoughts(new_thoughts[i-1], new_thoughts[i])
            if i < len(new_thoughts) - 1:
                self.connect_thoughts(new_thoughts[i], new_thoughts[i+1])

            for existing_thought in self.thoughts.values():
                if existing_thought not in new_thoughts:
                    self.connect_thoughts(new_thoughts[i], existing_thought, strength=0.1)

        return f"Processed input and added {len(new_thoughts)} new thoughts."

    def generate_response(self):
        if not self.high_trust_thoughts:
            return "I need more information to generate a response."
        
        selected_thought_id = max(self.high_trust_thoughts, key=lambda id: self.thought_counter[id])
        selected_thought = self.thoughts[selected_thought_id]
        response = [selected_thought.concept]

        for _ in range(random.randint(2, 5)):
            if selected_thought.connections:
                next_thought = random.choices(
                    list(selected_thought.connections.keys()),
                    weights=[strength for strength in selected_thought.connections.values()],
                    k=1
                )[0]
                response.append(next_thought.concept)
                selected_thought = next_thought
            else:
                break

        return " ".join(response).capitalize() + "."

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
                
                time.sleep(5)  # Adjust as needed
            except Exception as e:
                logging.error(f"Error in continuous learning loop: {e}")
                print(f"Error in continuous learning: {e}")
                time.sleep(10)

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

    def to_json(self):
        return {
            "nodes": [
                {
                    "id": thought.id,
                    "concept": thought.concept,
                    "trust": thought.trust
                } for thought in self.thoughts.values()
            ],
            "links": [
                {
                    "source": thought.id,
                    "target": connected.id,
                    "strength": strength
                } for thought in self.thoughts.values()
                  for connected, strength in thought.connections.items()
            ]
        }

    def emit_network_state(self):
        network_state = self.to_json()
        socketio.emit('network_update', network_state)

    def update_thought_metadata(self, thought_id, metadata):
        if thought_id in self.thoughts:
            self.thoughts[thought_id].metadata.update(metadata)
            self.save_thoughts()
            return True
        return False

app = Flask(__name__, static_folder='static')
socketio = SocketIO(app, cors_allowed_origins="*")

# Create a global instance of ThoughtNetwork
network = ThoughtNetwork()

@app.route('/')
def index():
    return render_template('index.html')

@socketio.on('connect')
def handle_connect():
    print('Client connected')
    network.emit_network_state()

def update_network():
    while True:
        network.emit_network_state()
        time.sleep(1)  # Adjust the interval as needed

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
        elif user_input == "\x1b[H":  
            network_thought = network.generate_response()
            print("Network asks Ollama:", network_thought)
            ollama_response = network.interact_with_ollama()
            print("Ollama responds:", ollama_response)
            print("Network's response:", network.generate_response())
        else:
            print(network.process_input(user_input))
            print("Network's response:", network.generate_response())
        network.emit_network_state()

if __name__ == '__main__':
    # Start the background network update thread
    update_thread = threading.Thread(target=update_network)
    update_thread.daemon = True
    update_thread.start()

    # Start the terminal interface in a separate thread
    terminal_thread = threading.Thread(target=terminal_interface)
    terminal_thread.daemon = True
    terminal_thread.start()

    # Start the Flask application in the main thread
    socketio.run(app, debug=True, use_reloader=False, port=5000)