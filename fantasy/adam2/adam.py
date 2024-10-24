import networkx as nx
import random
import json
from flask import Flask, request, jsonify, send_file
from flask_cors import CORS
from flask_socketio import SocketIO, emit
import sqlite3
import logging
from collections import defaultdict
from logging.handlers import RotatingFileHandler
from networkx.readwrite import json_graph

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
    def __init__(self, db_name='adam.sqlite'):
        self.db_name = db_name
        self.trust_threshold = 0.5
        self.concept_graph = nx.MultiDiGraph()
        self.init_db()
        self.load_graph_from_db()

    def init_db(self):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute('''CREATE TABLE IF NOT EXISTS concepts
                         (id INTEGER PRIMARY KEY, name TEXT UNIQUE)''')
            c.execute('''CREATE TABLE IF NOT EXISTS relationships
                         (id INTEGER PRIMARY KEY, 
                          concept1_id INTEGER, 
                          concept2_id INTEGER,
                          FOREIGN KEY(concept1_id) REFERENCES concepts(id),
                          FOREIGN KEY(concept2_id) REFERENCES concepts(id))''')
            c.execute('''CREATE TABLE IF NOT EXISTS trust_values
                         (id INTEGER PRIMARY KEY,
                          relationship_id INTEGER,
                          tag TEXT,
                          trust REAL,
                          FOREIGN KEY(relationship_id) REFERENCES relationships(id))''')
            conn.commit()
        logging.info("Database initialized")

    def load_graph_from_db(self):
        with sqlite3.connect(self.db_name) as conn:
            c = conn.cursor()
            c.execute('''
                SELECT c1.name, c2.name, tv.tag, tv.trust
                FROM relationships r
                JOIN concepts c1 ON r.concept1_id = c1.id
                JOIN concepts c2 ON r.concept2_id = c2.id
                JOIN trust_values tv ON r.id = tv.relationship_id
            ''')
            relationships = c.fetchall()

        for c1, c2, tag, trust in relationships:
            self.concept_graph.add_edge(c1, c2, tag=tag, trust=trust)

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

# Initialize the AI
ai = TrustRailAI()

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

if __name__ == '__main__':
    logger.info("Starting the application")
    socketio.run(app, debug=True, port=5005, allow_unsafe_werkzeug=True)
