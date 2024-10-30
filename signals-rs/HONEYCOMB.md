Purpose:
Honeycomb is a decentralized, self-healing, veracity-aware, p2p system
for creating applications. It works around the need for a DHT or central authority,
using gossip and geometric properties to provide dynamic, responsive
routing and self-healing.

It uses defederation as a core mechanism to prevent misbehaviour.

Properties:
- Nodes can enter or leave at any time, and the system will resist
damage and Sybil attacks up to the limits of what is logically possible.
- Extremely efficient routing system without a central authority or even consensus
- Nodes can dynamically adjust their position to optimize routing and latency
- Nodes can self-heal from damage by gossiping with neighbours
- Nodes can self-organize into a hexagonal grid to optimize routing efficiency
- Nodes can self-organize into a hierarchical trust system to optimize routing efficiency
- Behaviour is tracked and judged locally, so nodes cannot misbehave without being defederated.

## 1. Hexagonal Grid Setup and Coordinate System

- **3D Coordinate System**:
  ```rust
  struct Position {
      x: i64,
      y: i64,
      z: i64,
  }

  impl Position {
      fn is_valid(&self) -> bool {
          self.x + self.y + self.z == 0
      }

      fn hex_distance(&self, other: &Position) -> u64 {
          ((self.x - other.x).abs() + 
           (self.y - other.y).abs() + 
           (self.z - other.z).abs()) / 2
      }
  }
  ```
  - Each node maintains its position as `(x: i64, y: i64, z: i64)` in their Iroh document
  - Constraint: x + y + z = 0 (ensures valid hex coordinates in 3D space)
  - Initial position is always (0,0,0) relative to first connected neighbor

- **Position Discovery and Updates**:
  ```rust
  struct NodeState {
      position: Position,
      neighbors: HashMap<NodeId, RelativePosition>,
      last_update: Timestamp,
  }

  struct RelativePosition {
      dx: i64,
      dy: i64,
      dz: i64,
      last_seen: Timestamp,
  }
  ```
  - Node position updates are stored in Iroh K/V as: `{node_id}/position -> (x,y,z)`
  - Nodes continuously adjust their global position by:
    1. Reading neighbor positions from their documents
    2. Calculating relative distances
    3. Updating their position to maintain consistent spatial relationships

---
**Test Coverage for Hexagonal Grid [HEX]**:

Core Position Validation:
- HEX-1000: Position constraints enforce x + y + z = 0
- HEX-1001: Position creation rejects invalid coordinates
- HEX-1002: Position modification maintains constraints
- HEX-1003: Position serialization/deserialization preserves validity

Distance Calculations:
- HEX-1100: Hex distance matches geometric reality
- HEX-1101: Distance calculations are symmetrical
- HEX-1102: Triangle inequality holds for all positions
- HEX-1103: Distance to self is always zero
- HEX-1104: Adjacent hexes always have distance 1

Position Management:
- HEX-1200: Initial position is (0,0,0) for first node
- HEX-1201: Position updates persist correctly in Iroh
- HEX-1202: Position updates trigger neighbor notifications
- HEX-1203: Concurrent position updates resolve consistently
- HEX-1204: Position history maintains temporal ordering

Neighbor Relationships:
- HEX-1300: Relative positions track temporal data accurately
- HEX-1301: Stale neighbor data is detected and handled
- HEX-1302: Position updates maintain neighbor consistency
- HEX-1303: Neighbor position changes trigger local updates
- HEX-1304: Invalid neighbor updates are rejected
---

## 2. Local Neighbor Discovery and Dynamic Pathing

- **Purpose
  - Allow for global routing using local information
  - Allow for dynamic pathing and load balancing
  - Allow for self-healing through gossip-based damage detection

- **Neighbor Table Structure**:
  ```rust
  struct NeighborTable {
      neighbors: Vec<(NodeId, RelativePosition)>,
      last_update: Timestamp,
  }

  struct RelativePosition {
      dx: i64,
      dy: i64,
      dz: i64,
      last_seen: Timestamp,
  }

  struct RoutingTable {
      routes: HashMap<TargetId, NextHopId>,
      distances: HashMap<TargetId, u64>,  // Cached hex distances
  }
  ```
  - Stored in Iroh K/V as: `{node_id}/neighbors -> Vec<(NodeId, RelativePosition)>`
  - RelativePosition: `struct { dx: i64, dy: i64, dz: i64, last_seen: Timestamp }`

- **Path Discovery**:
  - Each node maintains a routing table: `{node_id}/routes -> HashMap<TargetId, NextHopId>`
  - Path selection uses hex distance: `|x1-x2| + |y1-y2| + |z1-z2| / 2`
  - No explicit latency tracking - routes naturally optimize through position updates
  - Neighbours should dynamically adjust paths if neighbours disconnect or new, lower-latency paths become available.
---

**Test Coverage for Neighbor Discovery [NBR]**:

Neighbor Table Management:
- NBR-2000: Neighbor tables accurately track relative positions
- NBR-2001: Neighbor entries include valid timestamps
- NBR-2002: Stale neighbors are detected and pruned
- NBR-2003: Neighbor table persists across node restarts
- NBR-2004: Concurrent neighbor updates resolve consistently

Path Discovery:
- NBR-2100: Path selection optimizes for hex distance
- NBR-2101: Path updates trigger when neighbors disconnect
- NBR-2102: Better paths cause route table updates
- NBR-2103: Path selection respects node coherence ratings
- NBR-2104: Invalid paths are rejected and removed

Route Management:
- NBR-2200: Route tables maintain consistent state
- NBR-2201: Route updates propagate to affected nodes
- NBR-2202: Route cycles are detected and prevented
- NBR-2203: Route table handles concurrent updates
- NBR-2204: Obsolete routes are cleaned up

Performance Optimization:
- NBR-2300: Route selection prefers lower latency paths
- NBR-2301: Load balancing distributes traffic across routes
- NBR-2302: Route caching improves lookup performance
- NBR-2303: Route updates minimize network overhead
- NBR-2304: Path discovery scales with network size

Failure Handling:
- NBR-2400: Node disconnections trigger route updates
- NBR-2401: Network partitions are detected and handled
- NBR-2402: Route table recovers from corruption
- NBR-2403: Invalid updates are rejected gracefully
- NBR-2404: System maintains consistency during failures
---

## 3. Self-Healing Through Gossip-Based Damage Detection

- **Gossip Protocol Purpose**:
  - Enable decentralized network health monitoring
  - Provide rapid detection of node failures or network partitions
  - Support autonomous route maintenance without central coordination
  - Allow nodes to maintain accurate network topology views

- **Gossip Mechanism**:
  - Nodes periodically broadcast their known network state to neighbors
  - Information shared includes:
    1. Known active neighbors and their last seen timestamps
    2. Recent routing changes and their causes
    3. Position verification data
    4. Network partition warnings
  - Updates propagate only through trusted paths
  - Gossip frequency adapts to network stability

- **Automatic Rerouting**:
  - Network self-heals through:
    1. Continuous monitoring of neighbor activity
    2. Immediate propagation of failure detection
    3. Automatic path recalculation around dead zones
    4. Load-based route rebalancing
  - No explicit "down" status needed - inactivity naturally triggers rerouting
  - Position updates trigger route recalculation
  - Network automatically rebalances under load

- **Gossip Update Structure**:
```rust
struct GossipUpdate {
    source_id: NodeId,
    timestamp: Timestamp,
    position: Position,
    known_neighbors: Vec<(NodeId, LastSeen)>,
    route_updates: Vec<RouteChange>,
}

struct RouteChange {
    target: NodeId,
    new_next_hop: Option<NodeId>,
    reason: UpdateReason,
}

enum UpdateReason {
    NodeUnreachable { last_seen: Timestamp },
    BetterPath { distance: u64 },
    TrustChange { new_trust: f32 },
}
```

---
**Test Coverage for Gossip Protocol [GSP]**:

Gossip Mechanics:
- GSP-3000: Gossip updates propagate through trusted paths only
- GSP-3001: Gossip frequency adapts to network stability
- GSP-3002: Updates include all required network state components
- GSP-3003: Gossip messages are properly serialized/deserialized
- GSP-3004: Large gossip updates are chunked appropriately

Failure Detection:
- GSP-3100: Node failures are detected within specified time window
- GSP-3101: False positives are minimized in failure detection
- GSP-3102: Network partitions are identified correctly
- GSP-3103: Multiple simultaneous failures are handled properly
- GSP-3104: Failure detection works across trust boundaries

Self-Healing:
- GSP-3200: Dead zones trigger automatic path recalculation
- GSP-3201: Network rebalances under varying loads
- GSP-3202: Recovery actions maintain network consistency
- GSP-3203: Healing processes don't overwhelm network
- GSP-3204: Multiple healing actions coordinate properly

Update Propagation:
- GSP-3300: Updates reach all relevant nodes efficiently
- GSP-3301: Duplicate updates are properly deduplicated
- GSP-3302: Update ordering is maintained where necessary
- GSP-3303: Conflicting updates resolve consistently
- GSP-3304: Update propagation respects trust boundaries

Performance & Scaling:
- GSP-3400: Gossip overhead scales sublinearly with network size
- GSP-3401: Memory usage remains bounded during operation
- GSP-3402: CPU usage remains reasonable during updates
- GSP-3403: Network bandwidth usage is optimized
- GSP-3404: System handles high update frequencies

Security & Trust:
- GSP-3500: Malicious gossip is detected and blocked
- GSP-3501: Trust boundaries prevent update manipulation
- GSP-3502: Update authenticity is verified
- GSP-3503: System resists gossip-based attacks
- GSP-3504: Privacy of node state is maintained

---

## 4. Coherence-Based Network View

- **Dynamic Ticket Issuance Through Gossip**:
  - Track which nodes are introducing new peers through routing patterns and initial connections
  - Observe network growth patterns through gossip data
  - Nodes can identify the "sponsors" of new nodes through connection patterns
  - Excessive introduction of problematic nodes affects coherence ratings of the introducing node

- **Self-Regulating Defederation**:
  - Network naturally isolates nodes that introduce too many problematic peers
  - Responsibility for new node behavior partially falls on introducing node
  - Bad behavior by introduced nodes reduces coherence with their sponsor
  - Creates natural pressure against reckless network growth

- **Coherence Storage and Propagation**:
  - Coherence vectors stored as: `{node_id}/coherence -> HashMap<NodeId, f32>`  // -1.0 to 1.0
  - Each node only writes its own coherence ratings
  - Coherence changes trigger routing table updates
  - Network view is implicitly filtered by coherence ratings
  - No global coherence threshold - each node's view is unique

- **Dynamic Coherence Adjustment**:
  - Coherence scores adjust based on observed behavior:
    1. Consistent position updates increase coherence
    2. Valid route sharing increases coherence
    3. Successful message routing increases coherence
    4. Invalid updates decrease coherence
    5. Introducing problematic nodes decreases coherence
  - Coherence changes are gradual unless severe violation occurs
  - Updates from incoherent nodes are ignored, not blocked

Implementation details:
```rust
struct NodeIntroduction {
    new_node: NodeId,
    first_contact: NodeId,  // The node they initially connected through
    timestamp: Timestamp,
    initial_routes: Vec<NodeId>,
}

struct CoherenceStore {
    ratings: HashMap<NodeId, f32>,
    last_updated: HashMap<NodeId, Timestamp>,
    coherence_history: VecDeque<CoherenceEvent>,
}

enum CoherenceUpdateReason {
    PositionConsistency { duration: Duration },
    RouteValidity { success_count: u32 },
    MessageDelivery { success_rate: f32 },
    ProblemNode { introduced_id: NodeId },
    InvalidUpdate { details: String },
}
```
---
**Test Coverage for Coherence-Based Network View [COH]**:

Dynamic Ticket Management:
- COH-4000: Track node introduction patterns accurately
- COH-4001: Identify sponsor relationships correctly
- COH-4002: Record initial connection patterns
- COH-4003: Detect excessive node introductions
- COH-4004: Track problematic introduction chains

Defederation Mechanics:
- COH-4100: Isolate nodes introducing problematic peers
- COH-4101: Apply sponsor responsibility correctly
- COH-4102: Propagate coherence reductions appropriately
- COH-4103: Handle multiple defederation events consistently
- COH-4104: Prevent defederation feedback loops

Coherence Storage:
- COH-4200: Maintain accurate coherence vectors
- COH-4201: Handle concurrent coherence updates
- COH-4202: Persist coherence state correctly
- COH-4203: Trigger route updates on coherence changes
- COH-4204: Filter network view based on coherence

Dynamic Adjustment:
- COH-4300: Increase coherence for consistent position updates
- COH-4301: Reward valid route sharing appropriately
- COH-4302: Credit successful message routing
- COH-4303: Penalize invalid updates correctly
- COH-4304: Handle problematic node introductions

Temporal Management:
- COH-4400: Track coherence history accurately
- COH-4401: Apply gradual changes appropriately
- COH-4402: Handle severe violations immediately
- COH-4403: Maintain temporal consistency in updates
- COH-4404: Prune outdated coherence data

Security & Privacy:
- COH-4500: Prevent coherence manipulation attacks
- COH-4501: Protect coherence data privacy
- COH-4502: Validate coherence update authenticity
- COH-4503: Detect malicious coherence patterns
- COH-4504: Handle attempted gaming of system

Integration Testing:
- COH-4600: Coherence affects routing decisions correctly
- COH-4601: Network view remains consistent across updates
- COH-4602: System handles network partitions gracefully
- COH-4603: Coherence system scales with network size
- COH-4604: Recovery from extreme coherence events works
---

## 5. Signed Updates and Node Accountability

- **Public R/W Ticket System**:
  - Assign each node a unique **read/write ticket** that allows it to sign updates to its own data in a shared document
  - Each update to a node's information (e.g., position changes, neighbor updates) must be signed with its ticket
  - Only authorized nodes can modify their own data

- **Document-Based Coherence Records**:
  - Implement a shared document where each node records updates signed with its ticket
  - Nodes periodically check neighbors' update histories
  - Build coherence over time based on consistent behavior
  - Nodes with frequent defederation records are treated with lower coherence

Implementation details:
```rust
struct SignedUpdate {
    node_id: NodeId,
    timestamp: Timestamp,
    content: UpdateContent,
}

enum UpdateContent {
    Position(Position),
    NeighborList(Vec<NodeId>),
    CoherenceUpdate { target: NodeId, new_value: f32 },
    RouteChange(RouteUpdate),
}
```
---
**Test Coverage for Signed Updates and Node Accountability [SIG]**:

Update Verification:
- SIG-5000: Position updates maintain grid consistency
- SIG-5001: Neighbor list changes reflect actual connections
- SIG-5002: Coherence updates follow behavior rules
- SIG-5003: Route changes match network topology
- SIG-5004: Update timestamps maintain causal ordering

Document History:
- SIG-5100: Node behavior history accurately tracks patterns
- SIG-5101: Defederation events are properly recorded
- SIG-5102: Update frequency stays within bounds
- SIG-5103: Bad behavior patterns are detectable
- SIG-5104: History pruning preserves important events

Network Impact:
- SIG-5200: Updates propagate efficiently to relevant nodes
- SIG-5201: Network load stays reasonable under updates
- SIG-5202: Update conflicts resolve consistently
- SIG-5203: System handles node churn gracefully
- SIG-5204: Large-scale updates don't overwhelm network
---

## 6. Adaptive Coherence-Based Mobility

- **Position Optimization**:
  - Nodes naturally seek positions that optimize their latency to neighbors
  - Available positions in the hex grid fill based on actual network topology
  - No artificial "core" or "periphery" - just natural network geometry
  - Position changes require neighbor acceptance based on coherence

- **Neighbor Acceptance**:
  - Existing nodes must accept new neighbors based on observed behavior
  - Coherence ratings determine willingness to accept new neighbors
  - Natural filtering of unreliable nodes through selective neighbor acceptance
  - Grid positions fill organically based on merit and network needs

- **Pack-Up-and-Move Mechanism**:
  - Nodes can relocate when current position becomes suboptimal
  - Must earn acceptance from new neighbors through demonstrated reliability
  - No guaranteed acceptance in new locations - must prove worth
  - Natural resistance to bad actors through earned neighbor relationships

Implementation details:
```rust
struct NodeMobility {
    current_position: Position,
    target_position: Option<Position>,
    movement_state: MovementState,
    last_move: Timestamp,
}

enum MovementState {
    Stable,
    Seeking { target_latency: Duration },
    Relocating { reason: RelocationReason },
}

enum RelocationReason {
    LatencyOptimization,
    NetworkPartition,
    DefederationResponse,
}
```
---
**Test Coverage for Adaptive Mobility [MOB]**:

Position Optimization:
- MOB-6000: Nodes seek optimal positions based on neighbor latency
- MOB-6001: Grid positions reflect actual network topology
- MOB-6002: Position changes respect coherence thresholds
- MOB-6003: Network geometry emerges naturally
- MOB-6004: Position conflicts resolve consistently

Neighbor Acceptance:
- MOB-6100: Neighbor acceptance based on observed behavior
- MOB-6101: Coherence ratings influence acceptance decisions
- MOB-6102: Unreliable nodes filtered through selective acceptance
- MOB-6103: Grid position assignment follows merit
- MOB-6104: Acceptance decisions maintain network stability

Relocation Mechanics:
- MOB-6200: Suboptimal position triggers relocation evaluation
- MOB-6201: Relocation requires earned neighbor trust
- MOB-6202: Bad actors face increasing movement resistance
- MOB-6203: Network maintains stability during relocations
- MOB-6204: Multiple concurrent relocations resolve safely

Movement States:
- MOB-6300: Stable state maintains position efficiently
- MOB-6301: Seeking state optimizes for target latency
- MOB-6302: Relocation handles various trigger reasons
- MOB-6303: State transitions occur appropriately
- MOB-6304: Movement history tracks patterns accurately

Network Adaptation:
- MOB-6400: Network topology adapts to usage patterns
- MOB-6401: Load distribution remains balanced
- MOB-6402: Position changes improve overall efficiency
- MOB-6403: System resists topology manipulation
- MOB-6404: Adaptation preserves network coherence
---

## 7. Latency-Aware Path Optimization

- **Latency Proofs for Neighbor Selection**:
  - Implement VDF-based (Verifiable Delay Function) latency proofs between nodes
  - Each node measures and commits to its processing power during setup
  - Distance bounding uses parallel VDFs to create verifiable latency measurements
  - Proofs are publicly verifiable and resistant to spoofing
  - Nodes dynamically adjust positioning based on verified latency proofs
  - Nodes with stable, low-latency connections are favored as routing paths
  - Improves network efficiency and overall connectivity

- **Dynamic "Gravity" Effect**:
  - Use verified latency data to create a "gravity" effect
  - Nodes naturally cluster closer to low-latency neighbors
  - Enhances routing efficiency
  - Minimizes cross-network latency

- **Path Quality Metrics**:
  - Monitor round-trip times using parallel VDF races
  - Track packet loss and connection stability
  - Adjust routing preferences based on verified performance
  - Natural optimization through usage patterns

- **Proof of Latency Protocol Flow**:
  1. Setup: Node measures and commits to its processing power
  2. Vector Commitment: Node creates commitment using public key and processing power
  3. Distance Bounding: Nodes race two parallel VDFs to measure round-trip time
  4. Verification: Results provide proof of actual network latency

- **Latency-Based Positioning**:
  - Nodes use verified latency proofs to optimize their grid positions
  - Natural clustering emerges around low-latency connections
  - Position changes require new latency proofs with potential neighbors
  - Network topology naturally optimizes for minimal latency paths

Implementation details:
```rust
struct LatencyProof {
    prover: NodeId,
    verifier: NodeId,
    timestamp: Timestamp,
    processing_power: u64,
    vdf_result: VdfOutput,
    measured_latency: Duration,
}

struct VdfOutput {
    iterations: u64,
    result: Vec<u8>,
    proof: Vec<u8>,
}

struct LatencyMetrics {
    node_id: NodeId,
    round_trip: Duration,
    jitter: Duration,
    last_updated: Timestamp,
    stability_score: f32,
}

struct PathQuality {
    latency_history: VecDeque<LatencyMetrics>,
    preferred_routes: HashMap<NodeId, Vec<NodeId>>,
    route_scores: HashMap<NodeId, f32>,
}
```
---
**Test Coverage for Latency-Aware Path Optimization [LAT]**:

VDF Latency Proofs:
- LAT-7000: VDF races accurately measure round-trip time
- LAT-7001: Parallel VDF execution prevents gaming
- LAT-7002: Processing power commitments are accurate
- LAT-7003: Latency proofs resist spoofing attempts
- LAT-7004: Proof verification is computationally reasonable

Gravity Effect:
- LAT-7100: Nodes cluster based on verified latency
- LAT-7101: Clustering improves overall network efficiency
- LAT-7102: Natural topology emerges from latency data
- LAT-7103: System resists artificial clustering attempts
- LAT-7104: Cluster stability improves with time

Path Quality:
- LAT-7200: Round-trip measurements are accurate
- LAT-7201: Packet loss detection works correctly
- LAT-7202: Connection stability scoring is meaningful
- LAT-7203: Route preferences adapt to performance
- LAT-7204: Quality metrics resist manipulation

Position Optimization:
- LAT-7300: Grid positions optimize for proven latency
- LAT-7301: Position changes require valid proofs
- LAT-7302: Network topology minimizes overall latency
- LAT-7303: Optimization respects coherence boundaries
- LAT-7304: System handles conflicting optimizations

Performance:
- LAT-7400: VDF computation overhead is reasonable
- LAT-7401: Proof verification scales with network size
- LAT-7402: Latency optimization improves message delivery
- LAT-7403: System handles high-frequency updates
- LAT-7404: Resource usage remains bounded
---

## 8. Exponential Defederation Backoff and Self-Healing

- **Local Exponential Backoff**:
  - Each node tracks its own defederation history with individual neighbors
  - When a neighbor defederates you, double the waiting time before attempting to reconnect to THAT neighbor
  - Each neighbor maintains their own view of your behavior
  - Natural rate-limiting through local reputation building

- **Localized Self-Healing**:
  - When defederated by a neighbor, node immediately:
    1. Updates its local routing table to remove that path
    2. Notifies remaining neighbors of the path change
    3. Attempts to optimize position with remaining neighbors
  - No global coordination needed - just local adjustments

Implementation details:
```rust
struct LocalNeighborState {
    node_id: NodeId,
    connection_attempts: Vec<Timestamp>,
    backoff_duration: Duration,  // Doubles with each rejection
    last_defederation: Option<Timestamp>,
}

impl LocalNeighborState {
    fn can_attempt_reconnect(&self, now: Timestamp) -> bool {
        match self.last_defederation {
            Some(time) => (now - time) > self.backoff_duration,
            None => true
        }
    }
}
```
---
**Test Coverage for Exponential Backoff and Self-Healing [BAK]**:

Backoff Mechanics:
- BAK-8000: Backoff duration doubles after each rejection
- BAK-8001: Backoff applies per-neighbor independently
- BAK-8002: Backoff reset works after successful reconnection
- BAK-8003: Backoff state persists across node restarts
- BAK-8004: Multiple rejection sources handled independently

Self-Healing:
- BAK-8100: Route tables update immediately on defederation
- BAK-8101: Neighbor notifications propagate efficiently
- BAK-8102: Position optimization triggers after defederation
- BAK-8103: Network maintains connectivity during healing
- BAK-8104: Local adjustments preserve global stability

Reputation Building:
- BAK-8200: Local reputation tracks reconnection history
- BAK-8201: Successful connections improve reputation
- BAK-8202: Failed attempts affect future backoff timing
- BAK-8203: Reputation recovery follows expected patterns
- BAK-8204: Reputation state maintains consistency

Network Resilience:
- BAK-8300: Network routes around defederated nodes
- BAK-8301: Service quality maintained during healing
- BAK-8302: Multiple concurrent defederations handled
- BAK-8303: System resists denial of service attempts
- BAK-8304: Recovery doesn't amplify network load

Performance:
- BAK-8400: Backoff calculations are efficient
- BAK-8401: State storage scales with network size
- BAK-8402: Healing process has bounded overhead
- BAK-8403: System handles rapid defederation events
- BAK-8404: Resource usage remains reasonable during recovery
---

## 9. Iroh Ticket-Based Network Growth

- **Purpose**:
  - Create a social layer of trust through ticket-based network access
  - Prevent Sybil attacks by requiring existing nodes to issue Iroh tickets
  - Enable responsible, controlled network expansion through Iroh's authentication system

- **Iroh Ticket Structure**:
  ```rust
  struct Ticket {
      issuer_id: NodeId,
      iroh_ticket: IrohTicket,  // Iroh's authentication ticket
      timestamp: u64,
      max_uses: u32,
      used_by: Vec<NodeId>
  }
  ```

- **Ticket Tracking**:
  - Stored in Iroh K/V as: `{node_id}/tickets_issued -> Vec<Ticket>`
  - Each node tracks tickets they've accepted: `{node_id}/ticket_used -> Option<Ticket>`
  - Network entry requires valid Iroh ticket from existing node
  - Nodes have limited tickets to encourage responsible onboarding

- **Anti-Spam Mechanics**:
  - Nodes monitor their neighbors' `tickets_issued` documents
  - Trust penalties trigger when neighbors:
    1. Issue too many Iroh tickets (e.g., >100 per week)
    2. Issue tickets to nodes that get widely defederated
    3. Create ticket chains that lead to spam behavior

- **Ticket-Based Trust**:
  - Initial trust score for new nodes: 0.0
  - Issuing node starts with small positive trust (0.1) for new node
  - Issuing node is partially accountable for new node's behavior
  - Bad behavior by ticket recipients affects issuer's trust score

---
**Test Coverage for Network Growth [GRO]**:

Growth Control:
- GRO-9000: Network growth rate stays within healthy bounds
- GRO-9001: Ticket issuance patterns reflect network health
- GRO-9002: Excessive ticket issuance triggers penalties
- GRO-9003: Network resists rapid expansion attacks
- GRO-9004: Growth patterns maintain network stability

Trust Propagation:
- GRO-9100: Initial trust scores set correctly
- GRO-9101: Issuer accountability works as designed
- GRO-9102: Bad behavior affects sponsor trust appropriately
- GRO-9103: Trust chains reflect introduction patterns
- GRO-9104: Trust penalties propagate accurately

Anti-Spam:
- GRO-9200: Detect ticket issuance abuse patterns
- GRO-9201: Identify problematic ticket chains
- GRO-9202: Track defederation patterns in growth
- GRO-9203: Prevent ticket-based spam attacks
- GRO-9204: Handle mass-defederation events

Network Health:
- GRO-9300: Monitor network growth metrics
- GRO-9301: Track introduction success rates
- GRO-9302: Measure network quality over time
- GRO-9303: Detect unhealthy growth patterns
- GRO-9304: Maintain growth/quality balance

Performance:
- GRO-9400: Growth tracking scales with network size
- GRO-9401: Introduction processing remains efficient
- GRO-9402: Trust calculations handle network scale
- GRO-9403: System manages high introduction rates
- GRO-9404: Resource usage stays bounded during growth
---
