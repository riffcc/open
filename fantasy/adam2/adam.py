import networkx as nx
import random
import threading
import json
from flask import Flask, request, jsonify, send_file
from flask_cors import CORS
from flask_socketio import SocketIO, emit
import sqlite3
import logging
from collections import defaultdict
from logging.handlers import RotatingFileHandler
from networkx.readwrite import json_graph
import nltk
from nltk import pos_tag, word_tokenize
from collections import Counter
import time

nltk.download('punkt', quiet=True)
nltk.download('averaged_perceptron_tagger', quiet=True)

app = Flask(__name__)
CORS(app)
socketio = SocketIO(app, cors_allowed_origins="*", async_mode='threading', logger=True, engineio_logger=True)

# Set up enhanced logging
logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)
handler = RotatingFileHandler('adam.log', maxBytes=10000, backupCount=3)
formatter = logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
handler.setFormatter(formatter)
logger.addHandler(handler)

class TrustRailAI:
    def __init__(self, db_name='trustnet.db'):
        self.db_name = db_name
        self.initialize_database()

    def initialize_database(self):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            
            # Create concepts table
            c.execute('''CREATE TABLE IF NOT EXISTS concepts
                         (id INTEGER PRIMARY KEY, name TEXT UNIQUE)''')
            
            # Create relationships table
            c.execute('''CREATE TABLE IF NOT EXISTS relationships
                         (id INTEGER PRIMARY KEY,
                          concept1_id INTEGER,
                          concept2_id INTEGER,
                          tag TEXT,
                          trust REAL,
                          FOREIGN KEY (
                          concept1_id) REFERENCES concepts(id),
                          FOREIGN KEY (
                          concept2_id) REFERENCES concepts(id))''')
            
            # Create trust_values table
            c.execute('''CREATE TABLE IF NOT EXISTS trust_values
                         (id INTEGER PRIMARY KEY,
                          relationship_id INTEGER,
                          tag TEXT,
                          trust REAL,
                          FOREIGN KEY(relationship_id) REFERENCES relationships(id))''')
            
            conn.commit()

    def add_concept(self, concept):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute("INSERT OR IGNORE INTO concepts (name) VALUES (?)", (concept,))
            conn.commit()
        logging.info(f"Added concept: {concept}")

    def add_trust_relationship(self, concept1, concept2, trust_values):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            self.add_concept(concept1)
            self.add_concept(concept2)
            c1_id = c.execute("SELECT id FROM concepts WHERE name = ?", (concept1,)).fetchone()[0]
            c2_id = c.execute("SELECT id FROM concepts WHERE name = ?", (concept2,)).fetchone()[0]
            
            c.execute('''INSERT OR REPLACE INTO relationships (concept1_id, concept2_id) 
                         VALUES (?, ?)''', (c1_id, c2_id))
            relationship_id = c.lastrowid
            
            for tag, trust in trust_values.items():
                c.execute('''INSERT OR REPLACE INTO trust_values (relationship_id, tag, trust) 
                             VALUES (?, ?, ?)''', (relationship_id, tag, trust))
            conn.commit()
        logging.info(f"Added trust relationship: {concept1} -> {concept2}, values: {trust_values}")
        self.load_graph_from_db()  # Reload the graph after adding a relationship

    def get_trust(self, concept1, concept2, tag):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute('''SELECT trust FROM trust_values
                         JOIN relationships ON trust_values.relationship_id = relationships.id
                         JOIN concepts c1 ON relationships.concept1_id = c1.id
                         JOIN concepts c2 ON relationships.concept2_id = c2.id
                         WHERE c1.name = ? AND c2.name = ? AND trust_values.tag = ?''',
                      (concept1, concept2, tag))
            result = c.fetchone()
            return result[0] if result else None

    def make_decision(self, query, tag=None):
        relevant_concepts = [node for node in self.concept_graph.nodes if query.lower() in node.lower()]
        
        if not relevant_concepts:
            return "I don't have enough information to make a decision about that."

        decision_paths = []
        for concept in relevant_concepts:
            if tag:
                paths = nx.single_source_dijkstra_path(self.concept_graph, concept, weight=lambda u, v, d: 1 - d.get('trust', 0) if d.get('tag') == tag else float('inf'))
            else:
                paths = nx.single_source_dijkstra_path(self.concept_graph, concept, weight=lambda u, v, d: 1 - d.get('trust', 0))
            decision_paths.extend(paths.values())

        if not decision_paths:
            return "I couldn't find any relevant connections to make a decision."

        best_path = max(decision_paths, key=lambda path: self.calculate_path_trust(path, tag))
        decision = f"Based on the relationship between {best_path[0]} and {best_path[-1]}, I think "
        decision += self.generate_decision_text(best_path, tag)

        return decision

    def calculate_path_trust(self, path, tag=None):
        total_trust = 0
        for i in range(len(path) - 1):
            edge_data = self.concept_graph.get_edge_data(path[i], path[i+1])
            if edge_data:
                for key, data in edge_data.items():
                    if tag is None or data.get('tag') == tag:
                        total_trust += data.get('trust', 0)
        return total_trust / (len(path) - 1) if len(path) > 1 else 0

    def generate_decision_text(self, path, tag=None):
        start_concept = path[0]
        end_concept = path[-1]
        path_trust = self.calculate_path_trust(path, tag)

        if path_trust > 0.7:
            return f"{start_concept} strongly implies {end_concept}."
        elif path_trust > 0.4:
            return f"{start_concept} suggests {end_concept}."
        else:
            return f"there might be a weak connection between {start_concept} and {end_concept}."

    def get_all_concepts(self):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute("SELECT DISTINCT name FROM concepts")
            concepts = [row[0] for row in c.fetchall()]
        return concepts

    def build_graph(self, tag):
        G = nx.DiGraph()
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute('''SELECT c1.name, c2.name, trust_values.trust
                         FROM relationships
                         JOIN concepts c1 ON relationships.concept1_id = c1.id
                         JOIN concepts c2 ON relationships.concept2_id = c2.id
                         JOIN trust_values ON relationships.id = trust_values.relationship_id
                         WHERE trust_values.tag = ?''', (tag,))
            for source, target, trust in c.fetchall():
                G.add_edge(source, target, trust=trust)
        return G

    def propagate_trust(self, G, start_concept, depth=3):
        visited = set()
        queue = [(start_concept, 1.0, 0)]
        
        while queue:
            concept, trust, current_depth = queue.pop(0)
            if current_depth > depth:
                break
            
            if concept not in visited:
                visited.add(concept)
                G.nodes[concept]['trust'] = max(G.nodes[concept].get('trust', 0), trust)
                for neighbor in G.neighbors(concept):
                    edge_trust = G[concept][neighbor]['trust']
                    propagated_trust = trust * edge_trust
                    queue.append((neighbor, propagated_trust, current_depth + 1))

    def get_graph_data(self, tag=None):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            if tag:
                c.execute('''
                    SELECT c1.name, c2.name, tv.tag, tv.trust
                    FROM relationships r
                    JOIN concepts c1 ON r.concept1_id = c1.id
                    JOIN concepts c2 ON r.concept2_id = c2.id
                    JOIN trust_values tv ON r.id = tv.relationship_id
                    WHERE tv.tag = ?
                ''', (tag,))
            else:
                c.execute('''
                    SELECT c1.name, c2.name, tv.tag, tv.trust
                    FROM relationships r
                    JOIN concepts c1 ON r.concept1_id = c1.id
                    JOIN concepts c2 ON r.concept2_id = c2.id
                    JOIN trust_values tv ON r.id = tv.relationship_id
                ''')
            
            relationships = c.fetchall()

        G = nx.MultiDiGraph()
        for c1, c2, tag, trust in relationships:
            G.add_edge(c1, c2, tag=tag, trust=trust)

        # Calculate node sizes based on degree
        for node in G.nodes():
            G.nodes[node]['size'] = 5 + G.degree(node)

        # Convert to JSON-serializable format
        data = json_graph.node_link_data(G)
        
        # Modify the data structure to match D3.js expectations
        nodes = [{"id": node['id'], "size": G.nodes[node['id']]['size']} for node in data['nodes']]
        links = [{"source": link['source'], "target": link['target'], 
                  "tag": link['tag'], "trust": link['trust']} for link in data['links']]

        return {"nodes": nodes, "links": links}

    def learn_from_experience(self, concept1, concept2, outcome, tag):
        current_trust = self.get_trust(concept1, concept2, tag) or 0
        new_trust = current_trust + 0.1 * (outcome - current_trust)
        self.add_trust_relationship(concept1, concept2, {tag: new_trust})
        self.load_graph_from_db()  # Reload the graph after learning

    def get_all_tags(self):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute("SELECT DISTINCT tag FROM trust_values WHERE tag != ''")
            tags = [row[0] for row in c.fetchall()]
        return tags

    def reinforce_decision(self, start_concept, end_concept, tag=None):
        reinforcement_value = 0.1  # You can adjust this value as needed
        path = nx.shortest_path(self.concept_graph, start_concept, end_concept)
        
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            for i in range(len(path) - 1):
                c1, c2 = path[i], path[i+1]
                if tag:
                    c.execute('''
                        UPDATE trust_values
                        SET trust = MIN(trust + ?, 1.0)
                        WHERE relationship_id = (
                            SELECT id FROM relationships
                            WHERE concept1_id = (SELECT id FROM concepts WHERE name = ?)
                            AND concept2_id = (SELECT id FROM concepts WHERE name = ?)
                        )
                        AND tag = ?
                    ''', (reinforcement_value, c1, c2, tag))
                else:
                    c.execute('''
                        UPDATE trust_values
                        SET trust = MIN(trust + ?, 1.0)
                        WHERE relationship_id = (
                            SELECT id FROM relationships
                            WHERE concept1_id = (SELECT id FROM concepts WHERE name = ?)
                            AND concept2_id = (SELECT id FROM concepts WHERE name = ?)
                        )
                    ''', (reinforcement_value, c1, c2))
            conn.commit()
        
        self.load_graph_from_db()  # Reload the graph after reinforcement
        logger.info(f"Reinforced trust between {start_concept} and {end_concept}")

    def think(self):
        """Re-evaluate all concepts and their relationships."""
        logger.info("AI is thinking...")
        
        # Get all concepts and their relationships
        concepts = self.get_all_concepts()
        relationships = self.get_all_relationships()

        # Simple example of re-evaluation:
        # Strengthen relationships between frequently co-occurring concepts
        concept_occurrences = defaultdict(int)
        for rel in relationships:
            concept_occurrences[rel['concept1']] += 1
            concept_occurrences[rel['concept2']] += 1

        for rel in relationships:
            if concept_occurrences[rel['concept1']] > 5 and concept_occurrences[rel['concept2']] > 5:
                new_trust = min(1.0, rel['trust'] + 0.1)
                self.update_relationship(rel['concept1'], rel['concept2'], rel['tag'], new_trust)

        # Generate new relationships based on transitive properties
        for c1 in concepts:
            for c2 in concepts:
                if c1 != c2:
                    common_neighbors = self.find_common_neighbors(c1, c2)
                    if common_neighbors:
                        avg_trust = sum(n['trust'] for n in common_neighbors) / len(common_neighbors)
                        self.add_or_update_relationship(c1, c2, "inferred", avg_trust)

        logger.info("Thinking process completed")

    def find_common_neighbors(self, concept1, concept2):
        # Implementation to find common neighbors between two concepts
        # This is a placeholder and should be implemented based on your data structure
        pass

    def add_or_update_relationship(self, concept1, concept2, tag, trust):
        # Implementation to add a new relationship or update an existing one
        # This is a placeholder and should be implemented based on your data structure
        pass

    def process_prompt(self, prompt):
        self.learn_from_sentence(prompt)
        
        response = f"I've learned from your input: '{prompt}'. "
        
        # Analyze the sentence structure
        tokens = word_tokenize(prompt)
        tagged = pos_tag(tokens)
        structure = ' '.join([tag for _, tag in tagged])
        
        response += f"The sentence structure is: {structure}. "
        
        # Identify some grammar concepts
        if any(tag.startswith('VB') for _, tag in tagged):
            response += "I noticed you used a verb. "
        if any(tag.startswith('NN') for _, tag in tagged):
            response += "I noticed you used a noun. "
        if any(tag == 'JJ' for _, tag in tagged):
            response += "I noticed you used an adjective. "
        
        # Provide some insights based on learned concepts
        common_words = self.get_common_words()
        if common_words:
            response += f"Some common words I've learned are: {', '.join(common_words)}. "
        
        return response

    def get_common_words(self, n=3):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute('''SELECT word, SUM(count) as total_count 
                         FROM grammar_concepts 
                         GROUP BY word 
                         ORDER BY total_count DESC 
                         LIMIT ?''', (n,))
            return [row[0] for row in c.fetchall()]

    def initialize_grammar_tables(self):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute('''CREATE TABLE IF NOT EXISTS grammar_concepts
                         (tag TEXT, word TEXT, count INTEGER, 
                          PRIMARY KEY (tag, word))''')
            c.execute('''CREATE TABLE IF NOT EXISTS sentence_structures
                         (structure TEXT PRIMARY KEY, count INTEGER)''')
            conn.commit()

    def learn_from_sentence(self, sentence):
        tokens = word_tokenize(sentence)
        tagged = pos_tag(tokens)
        
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            
            # Learn individual word types
            for word, tag in tagged:
                c.execute('''INSERT INTO grammar_concepts (tag, word, count) 
                             VALUES (?, ?, 1) 
                             ON CONFLICT(tag, word) 
                             DO UPDATE SET count = count + 1''', (tag, word.lower()))
            
            # Learn sentence structure
            structure = ' '.join([tag for _, tag in tagged])
            c.execute('''INSERT INTO sentence_structures (structure, count) 
                         VALUES (?, 1) 
                         ON CONFLICT(structure) 
                         DO UPDATE SET count = count + 1''', (structure,))
            
            conn.commit()
        
        # Attempt to identify grammar concepts
        self.identify_grammar_concepts(tagged)

    def identify_grammar_concepts(self, tagged_sentence):
        if any(tag.startswith('VB') for _, tag in tagged_sentence):
            self.add_or_update_relationship('sentence', 'verb', 'contains', 1.0)
        if any(tag.startswith('NN') for _, tag in tagged_sentence):
            self.add_or_update_relationship('sentence', 'noun', 'contains', 1.0)
        if any(tag == 'JJ' for _, tag in tagged_sentence):
            self.add_or_update_relationship('sentence', 'adjective', 'contains', 1.0)

    def get_all_relationships(self):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute('''SELECT c1.name, c2.name, r.tag, r.trust 
                         FROM relationships r
                         JOIN concepts c1 ON r.concept1_id = c1.id
                         JOIN concepts c2 ON r.concept2_id = c2.id''')
            return [{'concept1': row[0], 'concept2': row[1], 'tag': row[2], 'trust': row[3]} 
                    for row in c.fetchall()]

def thinking_process(ai):
    while True:
        ai.think()
        time.sleep(300)  # Think every 5 minutes

# Initialize the AI
ai = TrustRailAI()
thinking_thread = threading.Thread(target=thinking_process, args=(ai,))
thinking_thread.daemon = True
thinking_thread.start()

# Flask backend
@app.route('/')
def index():
    return send_file('index.html')

@socketio.on('connect')
def handle_connect(auth):
    logger.info('Client connected')
    tags = ai.get_all_tags()
    concepts = ai.get_all_concepts()
    graph_data = ai.get_graph_data()
    initial_data = {
        'tags': tags,
        'concepts': concepts,
        'graph': graph_data
    }
    logger.info(f"Sending initial data: {initial_data}")
    emit('initial_data', initial_data)
    socketio.start_background_task(send_updates)

def send_updates():
    while True:
        socketio.sleep(10)  # Check for updates every 10 seconds
        graph_data = ai.get_graph_data()
        socketio.emit('graph_update', graph_data)

@socketio.on('disconnect')
def handle_disconnect():
    logger.info('Client disconnected')

@socketio.on('make_decision')
def handle_make_decision(data):
    logger.info(f"Received make_decision request: {data}")
    query = data['query'].strip()
    tag = data.get('tag')
    if not query:
        emit('decision_result', {'result': "Please enter a query."})
        return
    try:
        result = ai.make_decision(query, tag)
        logger.info(f"Decision result: {result}")
        emit('decision_result', {'result': result})
    except Exception as e:
        logger.error(f"Error in make_decision: {str(e)}", exc_info=True)
        emit('decision_result', {'result': "An error occurred while processing your query."})

@socketio.on('learn')
def handle_learn(data):
    logger.info(f"Received learn request: {data}")
    concept1 = data['concept1']
    concept2 = data['concept2']
    tag = data['tag']
    outcome = data['outcome']
    try:
        ai.learn_from_experience(concept1, concept2, outcome, tag)
        graph_data = ai.get_graph_data(tag)
        logger.info(f"Updated graph data: {graph_data}")
        emit('graph_update', graph_data)
    except Exception as e:
        logger.error(f"Error in learn: {str(e)}",
                    exc_info=True)

@socketio.on('add_relationship')
def handle_add_relationship(data):
    logging.info(f"Received add_relationship request: {data}")
    concept1 = data['concept1']
    concept2 = data['concept2']
    tag = data['tag']
    trust = data['trust']
    ai.add_trust_relationship(concept1, concept2, {tag: trust})
    emit('graph_update', ai.get_graph_data(tag))
    emit('tags_update', {'tags': ai.get_all_tags()})

@socketio.on('get_graph')
def handle_get_graph(data):
    tag = data['tag']
    emit('graph_update', ai.get_graph_data(tag))

@socketio.on('reinforce_decision')
def handle_reinforce_decision(data):
    logger.info(f"Received reinforce_decision request: {data}")
    start_concept = data['start_concept']
    end_concept = data['end_concept']
    tag = data.get('tag')
    try:
        ai.reinforce_decision(start_concept, end_concept, tag)
        emit('reinforcement_result', {'result': 'Decision reinforced successfully'})
    except Exception as e:
        logger.error(f"Error in reinforce_decision: {str(e)}", exc_info=True)
        emit('reinforcement_result', {'result': 'An error occurred while reinforcing the decision'})

@socketio.on('prompt')
def handle_prompt(data):
    logger.info(f"Received prompt: {data}")
    query = data['query'].strip()
    if not query:
        emit('ai_response', {'result': "Please enter a prompt or question."})
        return
    try:
        result = ai.process_prompt(query)
        logger.info(f"AI response: {result}")
        emit('ai_response', {'result': result})
    except Exception as e:
        logger.error(f"Error in processing prompt: {str(e)}", exc_info=True)
        emit('ai_response', {'result': "An error occurred while processing your prompt."})

if __name__ == '__main__':
    logger.info("Starting the application")
    socketio.run(app, debug=True, port=5005, allow_unsafe_werkzeug=True)
