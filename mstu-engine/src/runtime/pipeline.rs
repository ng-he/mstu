use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use mstu_sdk::{PluginDescriptor, PluginHandle, ProcessContext, Schema, ValueKind};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

use crate::{
    log_debug, log_trace, log_warn,
    runtime::{
        mapper::Mapper,
        process::{self, ProcessData},
        schema,
    },
};

pub type PipelineId = usize;
pub type NodeId = usize;

/// Number of fields in a schema. A null schema means "no message".
fn schema_field_count(schema: *const Schema) -> usize {
    if schema.is_null() {
        return 0;
    }

    unsafe { (*schema).fields.len }
}

/// A plugin with no process input is driven by the engine, not by messages.
fn is_source(descriptor: &PluginDescriptor) -> bool {
    (descriptor.process_input_schema)().is_null()
}

/// How long a source waits before retrying when it is off or has nothing.
const IDLE_POLL: Duration = Duration::from_millis(5);

pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub plugin: PluginHandle,
    pub descriptor: &'static PluginDescriptor,

    pub running: Arc<AtomicBool>,

    /// Cleared to tell the task to leave its loop, so the handle stops being
    /// used before the plugin is released.
    pub alive: Arc<AtomicBool>,

    /// A node that fails does so every round, so failures are counted and
    /// reported sparsely instead of at message rate.
    pub failures: AtomicUsize,
}

unsafe impl Send for Node {}

pub struct Connector {
    pub to_node_id: NodeId,
    pub mapper: Mapper,

    /// Drops happen at message rate, so they are counted and reported
    /// sparsely instead of once per message.
    pub dropped: AtomicUsize,
}

pub struct Output {
    pub node_id: NodeId,
    pub data: ProcessData,
}

pub struct Worker {
    pub node_id: NodeId,

    /// Which plugin instance this node runs, so removing one finds its nodes.
    pub plugin: PluginHandle,

    /// Kept here too: the node itself moves into its task on start.
    pub name: String,

    pub tx: UnboundedSender<ProcessData>,

    /// Field count of the node process input schema, used to size the
    /// message a connector maps into.
    pub input_field_count: usize,

    /// Value kinds the node's input schema demands.
    pub input_kinds: Vec<ValueKind>,

    /// Shared with the node task, flipped to switch the node on and off.
    pub running: Arc<AtomicBool>,

    /// Shared with the node task, cleared to make it exit.
    pub alive: Arc<AtomicBool>,

    /// Awaited on removal: the task holds the plugin handle until it ends.
    pub task: JoinHandle<()>,

    pub is_source: bool,
}

pub struct Pipeline {
    pub id: PipelineId,
    pub name: String,

    pub nodes: HashMap<NodeId, Node>,
    pub workers: HashMap<NodeId, Worker>,
    pub connectors: HashMap<NodeId, Vec<Connector>>,
    pub parent_lookup: HashMap<NodeId, NodeId>,

    output_tx: UnboundedSender<Output>,
    output_rx: UnboundedReceiver<Output>,

    next_id: NodeId,
}

impl Pipeline {
    pub fn new(id: PipelineId, name: &str) -> Self {
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();

        Self {
            id,
            name: name.to_string(),

            nodes: HashMap::new(),
            workers: HashMap::new(),

            connectors: HashMap::new(),
            parent_lookup: HashMap::new(),

            output_tx,
            output_rx,

            next_id: 0,
        }
    }

    pub fn add_node(
        &mut self,
        plugin: PluginHandle,
        descriptor: &'static PluginDescriptor,
    ) -> NodeId {
        let node_id = self.next_id;
        self.next_id += 1;

        // Only a source generates work on its own, so only a source starts off.
        let node = Node {
            id: node_id,
            name: unsafe { (descriptor.metadata)().name.as_str() }.to_string(),
            plugin,
            descriptor,
            running: Arc::new(AtomicBool::new(!is_source(descriptor))),
            alive: Arc::new(AtomicBool::new(true)),
            failures: AtomicUsize::new(0),
        };

        self.nodes.insert(node_id, node);

        node_id
    }

    /// A node is in `nodes` until it starts and in `workers` after.
    pub fn has_node(&self, node_id: NodeId) -> bool {
        self.nodes.contains_key(&node_id) || self.workers.contains_key(&node_id)
    }

    /// False if either node is unknown. Connecting is allowed while running:
    /// the mapping just changes under the traffic.
    pub fn connect(&mut self, from_node_id: NodeId, to_node_id: NodeId, mapper: Mapper) -> bool {
        if !self.has_node(from_node_id) || !self.has_node(to_node_id) {
            return false;
        }

        let connectors = self.connectors.entry(from_node_id).or_default();

        // Connecting the same pair again replaces its mapping, and the new one
        // deserves a fresh drop count.
        match connectors
            .iter_mut()
            .find(|connector| connector.to_node_id == to_node_id)
        {
            Some(connector) => {
                connector.mapper = mapper;
                connector.dropped.store(0, Ordering::Relaxed);
            }
            None => connectors.push(Connector {
                to_node_id,
                mapper,
                dropped: AtomicUsize::new(0),
            }),
        }

        self.parent_lookup.insert(to_node_id, from_node_id);

        true
    }

    /// Drops the mapping filling `output` on the link between the two nodes.
    ///
    /// None when the pair is not connected, otherwise whether one was there.
    /// Unmapping is allowed while running, the same way connecting is.
    pub fn remove_mapping(
        &mut self,
        from_node_id: NodeId,
        to_node_id: NodeId,
        output: &[usize],
    ) -> Option<bool> {
        let connector = self
            .connectors
            .get_mut(&from_node_id)?
            .iter_mut()
            .find(|connector| connector.to_node_id == to_node_id)?;

        Some(connector.mapper.remove(output))
    }

    /// Drops the link between two nodes, and says whether one was there.
    ///
    /// Disconnecting is allowed while running: the output simply stops being
    /// routed on, the way connecting takes effect under the traffic.
    pub fn disconnect(&mut self, from_node_id: NodeId, to_node_id: NodeId) -> bool {
        let Some(connectors) = self.connectors.get_mut(&from_node_id) else {
            return false;
        };

        let before = connectors.len();
        connectors.retain(|connector| connector.to_node_id != to_node_id);

        if connectors.len() == before {
            return false;
        }

        // `start` reads this map to bring targets up first, so a node left with
        // an empty list must not still look like it feeds another.
        if connectors.is_empty() {
            self.connectors.remove(&from_node_id);
        }

        // Only if it still names this parent: the target may have been
        // reconnected to something else since.
        if self.parent_lookup.get(&to_node_id) == Some(&from_node_id) {
            self.parent_lookup.remove(&to_node_id);
        }

        true
    }

    /// Spawns every node and switches it on. Targets start before sources, so
    /// no output is routed to a node that has no worker yet.
    pub fn start(&mut self) {
        let mut node_ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        node_ids.sort_by_key(|node_id| self.connectors.contains_key(node_id));

        for node_id in node_ids {
            self.start_node(node_id);
        }

        for worker in self.workers.values() {
            worker.running.store(true, Ordering::Relaxed);
        }
    }

    pub fn start_node(&mut self, node_id: NodeId) {
        let node = self.nodes.remove(&node_id).expect("node not found");
        let output_tx = self.output_tx.clone();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ProcessData>();

        log_debug!(
            "pipeline {}: node {node_id} '{}' spawned as {}",
            self.id,
            node.name,
            if is_source(node.descriptor) {
                "source"
            } else {
                "worker"
            }
        );

        let worker = Worker {
            node_id,
            plugin: node.plugin,
            name: node.name.clone(),
            tx,
            input_field_count: schema_field_count((node.descriptor.process_input_schema)()),
            input_kinds: schema::expected_kinds((node.descriptor.process_input_schema)()),
            running: node.running.clone(),
            alive: node.alive.clone(),
            is_source: is_source(node.descriptor),

            task: match is_source(node.descriptor) {
                true => tokio::spawn(Self::run_source(node, output_tx)),
                false => tokio::spawn(Self::run_worker(node, rx, output_tx)),
            },
        };

        self.workers.insert(node_id, worker);
    }

    /// Node ids running a given plugin instance, started or not.
    pub fn nodes_of(&self, plugin: PluginHandle) -> Vec<NodeId> {
        let started = self
            .workers
            .values()
            .filter(|worker| worker.plugin == plugin)
            .map(|worker| worker.node_id);

        self.nodes
            .values()
            .filter(|node| node.plugin == plugin)
            .map(|node| node.id)
            .chain(started)
            .collect()
    }

    /// Unlinks a node and tells its task to stop, returning the task to await.
    ///
    /// The task holds the plugin handle, so the caller must await it before
    /// releasing the plugin.
    pub fn remove_node(&mut self, node_id: NodeId) -> Option<JoinHandle<()>> {
        self.connectors.remove(&node_id);

        for connectors in self.connectors.values_mut() {
            connectors.retain(|connector| connector.to_node_id != node_id);
        }

        self.parent_lookup.remove(&node_id);
        self.parent_lookup.retain(|_, parent| *parent != node_id);

        // Never started, so there is no task and nothing holds the handle.
        if let Some(node) = self.nodes.remove(&node_id) {
            node.alive.store(false, Ordering::Relaxed);
            return None;
        }

        let worker = self.workers.remove(&node_id)?;
        worker.alive.store(false, Ordering::Relaxed);

        // The rest of the worker, its sender included, drops here: that is what
        // ends a non-source task.
        Some(worker.task)
    }

    /// A source has no input to wait on: it produces while it is switched on.
    async fn run_source(node: Node, output_tx: UnboundedSender<Output>) {
        while node.alive.load(Ordering::Relaxed) {
            if !node.running.load(Ordering::Relaxed) {
                tokio::time::sleep(IDLE_POLL).await;
                continue;
            }

            let Some(data) = Self::process_node(&node, ProcessData::new(0)) else {
                // Nothing this round: end of stream, or no data yet.
                tokio::time::sleep(IDLE_POLL).await;
                continue;
            };

            if output_tx
                .send(Output {
                    node_id: node.id,
                    data,
                })
                .is_err()
            {
                break;
            }

            // The loop never awaits on its own, let the runtime breathe.
            tokio::task::yield_now().await;
        }
    }

    /// Every other node processes what arrives on its channel.
    async fn run_worker(
        node: Node,
        mut rx: UnboundedReceiver<ProcessData>,
        output_tx: UnboundedSender<Output>,
    ) {
        while let Some(input) = rx.recv().await {
            if !node.alive.load(Ordering::Relaxed) {
                break;
            }

            // Messages that arrive while the node is off are dropped.
            if !node.running.load(Ordering::Relaxed) {
                continue;
            }

            let Some(data) = Self::process_node(&node, input) else {
                continue;
            };

            if output_tx
                .send(Output {
                    node_id: node.id,
                    data,
                })
                .is_err()
            {
                break;
            }
        }
    }

    /// A node lives in `nodes` until it starts, then in `workers`.
    pub fn node_name(&self, node_id: NodeId) -> Option<&str> {
        if let Some(node) = self.nodes.get(&node_id) {
            return Some(node.name.as_str());
        }

        self.workers
            .get(&node_id)
            .map(|worker| worker.name.as_str())
    }

    pub fn set_node_name(&mut self, node_id: NodeId, name: &str) -> bool {
        let mut renamed = false;

        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.name = name.to_string();
            renamed = true;
        }

        if let Some(worker) = self.workers.get_mut(&node_id) {
            worker.name = name.to_string();
            renamed = true;
        }

        renamed
    }

    pub fn set_node_running(&self, node_id: NodeId, running: bool) -> bool {
        let Some(worker) = self.workers.get(&node_id) else {
            return false;
        };

        worker.running.store(running, Ordering::Relaxed);

        true
    }

    /// Switches the sources off and leaves the rest running, so whatever is
    /// still in flight drains instead of being dropped.
    pub fn stop(&self) {
        for worker in self.workers.values() {
            if worker.is_source {
                worker.running.store(false, Ordering::Relaxed);
            }
        }
    }

    fn process_node(node: &Node, input: ProcessData) -> Option<ProcessData> {
        // The engine owns every allocation: the output message is created here,
        // and the plugin only fills it through the writer.
        let mut data =
            ProcessData::new(schema_field_count((node.descriptor.process_output_schema)()));

        let mut writer = process::new_writer();
        writer._engine_data = (&mut data as *mut ProcessData).cast();

        let mut p_ctx = ProcessContext {
            input: &input.output,
            output: &mut writer,
        };

        log_trace!("node {} '{}' <- {:?}", node.id, node.name, input.output);

        if !(node.descriptor.process)(node.plugin, &mut p_ctx) {
            let failures = node.failures.fetch_add(1, Ordering::Relaxed) + 1;

            if failures == 1 || failures % 1000 == 0 {
                log_warn!(
                    "node {} '{}' failed to process ({failures} time(s))",
                    node.id,
                    node.name
                );
            }

            return None;
        }

        // Sink node, or nothing was produced this round (end of stream).
        if data.is_empty() {
            return None;
        }

        log_trace!("node {} '{}' -> {:?}", node.id, node.name, data.output);

        Some(data)
    }

    /// Warns about a dropped message without flooding: the first, then every
    /// thousandth.
    fn note_drop(&self, connector: &Connector, from_node_id: NodeId, reason: &str) {
        let dropped = connector.dropped.fetch_add(1, Ordering::Relaxed) + 1;

        if dropped == 1 || dropped % 1000 == 0 {
            log_warn!(
                "pipeline {}: dropped {dropped} message(s) {from_node_id} -> {}: {reason}",
                self.id,
                connector.to_node_id
            );
        }
    }

    fn route_output(&self, output: Output) {
        let Some(connectors) = self.connectors.get(&output.node_id) else {
            return;
        };

        for connector in connectors {
            // An unmapped connector carries nothing, and a target reading a
            // field the mapper never wrote is how plugins get killed.
            if connector.mapper.is_empty() {
                self.note_drop(connector, output.node_id, "nothing is mapped");
                continue;
            }

            let Some(worker) = self.workers.get(&connector.to_node_id) else {
                self.note_drop(connector, output.node_id, "target has no worker");
                continue;
            };

            /*
             * Mapper:
             *
             * source output
             *      │
             *      ▼
             * connector.mapper
             *      │
             *      ▼
             * target ProcessData
             */

            let mut data = ProcessData::new(worker.input_field_count);

            if !connector
                .mapper
                .map(&output.data.output, &mut data.output, &mut data.arena)
            {
                self.note_drop(connector, output.node_id, "mapping points at no such field");
                continue;
            }

            // A mapping that writes the wrong type would crash the target.
            if !schema::matches(&data.output, &worker.input_kinds) {
                self.note_drop(
                    connector,
                    output.node_id,
                    &format!("{:?} does not match the input schema", data.output),
                );

                continue;
            }

            log_trace!(
                "pipeline {}: routed {} -> {} '{}'",
                self.id,
                output.node_id,
                worker.node_id,
                worker.name
            );

            let _ = worker.tx.send(data);
        }
    }

    /// Routes whatever is queued right now, returns how many it routed.
    pub fn route_pending(&mut self) -> usize {
        let mut routed = 0;

        while let Ok(output) = self.output_rx.try_recv() {
            self.route_output(output);
            routed += 1;
        }

        routed
    }

    /// Routes node outputs until none arrives for `idle`, returns how many
    /// it routed.
    pub async fn run(&mut self, idle: Duration) -> usize {
        let mut routed = 0;

        loop {
            let output = match tokio::time::timeout(idle, self.output_rx.recv()).await {
                Ok(Some(output)) => output,
                _ => break,
            };

            self.route_output(output);
            routed += 1;
        }

        routed
    }

    pub fn send(&self, node_id: NodeId, data: ProcessData) -> bool {
        let Some(worker) = self.workers.get(&node_id) else {
            return false;
        };

        worker.tx.send(data).is_ok()
    }
}
