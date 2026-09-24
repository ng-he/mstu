use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use libloading::Library;
use mstu_sdk::{
    HostContext, Message, PluginDescriptor, PluginDescriptorFn, PluginHandle, Str, Value,
};

use crate::{
    log_info, log_warn, logging,
    runtime::{
        event::{self, Subscription},
        live,
        mapper::Mapper,
        pipeline::{NodeId, Pipeline, PipelineId},
        process::ProcessData,
        schema,
    },
    utils::generate_plugin_id,
};

pub type Result<T> = std::result::Result<T, String>;

pub struct Plugin {
    pub id: String,
    pub handle: PluginHandle,
    pub descriptor: &'static PluginDescriptor,

    /// Folder from `ui()`, resolved next to the plugin library.
    pub ui: Option<PathBuf>,
}

struct Loaded {
    descriptor: &'static PluginDescriptor,
    dir: PathBuf,
}

/// `Str::empty` is a null pointer, so it can never reach `as_str`.
fn optional_str(value: Str) -> Option<&'static str> {
    if value.ptr.is_null() || value.len == 0 {
        return None;
    }

    Some(unsafe { value.as_str() })
}

pub struct App {
    /// Boxed: plugins keep the pointer for their whole life.
    ctx: Box<HostContext>,

    descriptors: HashMap<String, Loaded>,
    plugins: HashMap<String, Plugin>,
    pipelines: HashMap<PipelineId, Pipeline>,

    next_pipeline_id: PipelineId,
}

impl App {
    pub fn new() -> Self {
        Self {
            ctx: Box::new(HostContext {
                logger: logging::log_impl,
                log_enabled: logging::enabled,
                new_event: event::new_event,
                publish_events: event::publish_events,
                publish_live: live::publish_live,
            }),

            descriptors: HashMap::new(),
            plugins: HashMap::new(),
            pipelines: HashMap::new(),

            next_pipeline_id: 0,
        }
    }

    /// Loads a plugin library under `name`.
    ///
    /// The library is leaked on purpose: descriptors and worker tasks point
    /// into its code for as long as the process runs.
    pub fn load(&mut self, name: &str, path: &str) -> Result<()> {
        let library = unsafe { Library::new(path) }.map_err(|err| err.to_string())?;
        let library: &'static Library = Box::leak(Box::new(library));

        let descriptor = unsafe {
            let plugin_descriptor = library
                .get::<PluginDescriptorFn>(b"plugin_descriptor")
                .map_err(|err| err.to_string())?;

            &*plugin_descriptor()
        };

        let dir = Path::new(path)
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        let metadata = (descriptor.metadata)();

        log_info!(
            "loaded library '{name}' ({} {}) from {path}",
            unsafe { metadata.name.as_str() },
            unsafe { metadata.version.as_str() }
        );

        self.descriptors
            .insert(name.to_string(), Loaded { descriptor, dir });

        Ok(())
    }

    /// Instantiates a plugin from a loaded library, returns its id.
    pub fn create_plugin(&mut self, library: &str) -> Result<String> {
        let loaded = self
            .descriptors
            .get(library)
            .ok_or_else(|| format!("library '{library}' not loaded"))?;

        let descriptor = loaded.descriptor;

        // Absolute, so a client with a different working directory can load it.
        // A plugin without a deployed ui folder just has no UI.
        let ui = optional_str((descriptor.ui)())
            .map(|folder| loaded.dir.join(folder))
            .and_then(|path| path.canonicalize().ok())
            .filter(|path| path.is_dir());

        let id = generate_plugin_id();

        // Both are open before the plugin exists, so it may publish from
        // `create` onwards.
        event::manager().register(&id, descriptor);
        live::board().register(&id, schema::shape_of((descriptor.live_schema)()));

        let handle = (descriptor.create)(&*self.ctx, Str::new(id.as_str()));

        match &ui {
            Some(path) => log_info!("created plugin '{id}' from '{library}', ui {}", path.display()),
            None => log_info!("created plugin '{id}' from '{library}', no ui"),
        }

        self.plugins.insert(
            id.clone(),
            Plugin {
                id: id.clone(),
                handle,
                descriptor,
                ui,
            },
        );

        Ok(id)
    }

    /// Takes a plugin out of every pipeline and releases it.
    ///
    /// Async because a worker task holds the plugin handle: it has to finish
    /// before the plugin can be released.
    pub async fn remove_plugin(&mut self, plugin_id: &str) -> Result<()> {
        let (handle, descriptor) = {
            let plugin = self.plugin(plugin_id)?;
            (plugin.handle, plugin.descriptor)
        };

        let mut tasks = Vec::new();

        for pipeline in self.pipelines.values_mut() {
            for node_id in pipeline.nodes_of(handle) {
                log_info!("pipeline {}: removed node {node_id}", pipeline.id);

                if let Some(task) = pipeline.remove_node(node_id) {
                    tasks.push(task);
                }
            }
        }

        for task in tasks {
            let _ = task.await;
        }

        // Nothing runs this plugin now, so the handle is free to release.
        event::manager().forget(plugin_id, handle);
        (descriptor.release)(handle);

        // After releasing: a plugin may report a last reading on its way out.
        live::board().forget(plugin_id);
        self.plugins.remove(plugin_id);

        log_info!("removed plugin '{plugin_id}'");

        Ok(())
    }

    /// Folder holding the plugin UI, if it has one.
    pub fn plugin_ui(&self, plugin_id: &str) -> Result<Option<&Path>> {
        Ok(self.plugin(plugin_id)?.ui.as_deref())
    }

    pub fn plugin(&self, plugin_id: &str) -> Result<&Plugin> {
        self.plugins
            .get(plugin_id)
            .ok_or_else(|| format!("plugin '{plugin_id}' not found"))
    }

    pub fn set_parameter(&self, plugin_id: &str, field: usize, value: Value) -> Result<()> {
        let plugin = self.plugin(plugin_id)?;

        // A client's number carries no type, so give the field the one it asks
        // for rather than whatever JSON happened to parse.
        let value = match schema::field_kind((plugin.descriptor.settings_schema)(), field) {
            Some(kind) => schema::coerce(value, kind),
            None => value,
        };

        log_info!("set parameter {field} of '{plugin_id}' to {value}");

        if !(plugin.descriptor.set_parameter)(plugin.handle, field, value) {
            log_warn!("plugin '{plugin_id}' refused parameter {field}");

            return Err(format!("set parameter {field} of '{plugin_id}' failed"));
        }

        Ok(())
    }

    pub fn invoke(&self, plugin_id: &str, command: usize, input: &Message) -> Result<()> {
        let plugin = self.plugin(plugin_id)?;

        log_info!("invoke command {command} on '{plugin_id}' with {input:?}");

        if !(plugin.descriptor.invoke)(plugin.handle, command, input) {
            log_warn!("command {command} of '{plugin_id}' failed");

            return Err(format!("command {command} of '{plugin_id}' failed"));
        }

        Ok(())
    }

    /// Invokes `command` on `to` whenever `from` publishes `event`.
    pub fn subscribe(
        &mut self,
        from: &str,
        event: usize,
        to: &str,
        command: usize,
        mapper: Mapper,
    ) -> Result<()> {
        let target = self.plugin(to)?;

        log_info!(
            "subscribe: '{from}' event {event} -> '{to}' command {command}, {} mapping(s)",
            mapper.len()
        );

        let subscription = Subscription::new(target.handle, target.descriptor, command, mapper);

        event::manager().subscribe(from, event, subscription);

        Ok(())
    }

    /// Stops invoking `command` on `to` when `from` publishes `event`.
    pub fn unsubscribe(&mut self, from: &str, event: usize, to: &str, command: usize) -> Result<()> {
        let target = self.plugin(to)?;
        let handle = target.handle;

        if !event::manager().unsubscribe(from, event, handle, command) {
            return Err(format!(
                "'{to}' command {command} is not subscribed to event {event} of '{from}'"
            ));
        }

        log_info!("unsubscribe: '{from}' event {event} -> '{to}' command {command}");

        Ok(())
    }

    pub fn create_pipeline(&mut self, name: &str) -> PipelineId {
        let pipeline_id = self.next_pipeline_id;
        self.next_pipeline_id += 1;

        log_info!("created pipeline {pipeline_id} '{name}'");

        self.pipelines
            .insert(pipeline_id, Pipeline::new(pipeline_id, name));

        pipeline_id
    }

    pub fn pipeline_mut(&mut self, pipeline_id: PipelineId) -> Result<&mut Pipeline> {
        self.pipelines
            .get_mut(&pipeline_id)
            .ok_or_else(|| format!("pipeline {pipeline_id} not found"))
    }

    pub fn add_node(&mut self, pipeline_id: PipelineId, plugin_id: &str) -> Result<NodeId> {
        let plugin = self
            .plugins
            .get(plugin_id)
            .ok_or_else(|| format!("plugin '{plugin_id}' not found"))?;

        let handle = plugin.handle;
        let descriptor = plugin.descriptor;

        let pipeline = self.pipeline_mut(pipeline_id)?;
        let node_id = pipeline.add_node(handle, descriptor);

        log_info!(
            "pipeline {pipeline_id}: added node {node_id} '{}' ('{plugin_id}')",
            pipeline.node_name(node_id).unwrap_or_default()
        );

        Ok(node_id)
    }

    pub fn connect(
        &mut self,
        pipeline_id: PipelineId,
        from_node_id: NodeId,
        to_node_id: NodeId,
        mapper: Mapper,
    ) -> Result<()> {
        let mappings = mapper.len();

        if !self
            .pipeline_mut(pipeline_id)?
            .connect(from_node_id, to_node_id, mapper)
        {
            log_warn!(
                "pipeline {pipeline_id}: cannot connect {from_node_id} -> {to_node_id}, unknown node"
            );

            return Err(format!("node {from_node_id} or {to_node_id} not found"));
        }

        log_info!(
            "pipeline {pipeline_id}: connected {from_node_id} -> {to_node_id}, {mappings} mapping(s)"
        );

        Ok(())
    }

    /// Drops the link between two nodes, mappings and all.
    ///
    /// Subscriptions are not part of a link, so they outlive it and are
    /// dropped on their own.
    pub fn disconnect(
        &mut self,
        pipeline_id: PipelineId,
        from_node_id: NodeId,
        to_node_id: NodeId,
    ) -> Result<bool> {
        let disconnected = self
            .pipeline_mut(pipeline_id)?
            .disconnect(from_node_id, to_node_id);

        if disconnected {
            log_info!("pipeline {pipeline_id}: disconnected {from_node_id} -> {to_node_id}");
        } else {
            log_warn!(
                "pipeline {pipeline_id}: {from_node_id} -> {to_node_id} was not connected"
            );
        }

        Ok(disconnected)
    }

    /// Drops one mapping from a link, leaving the rest of it alone.
    pub fn remove_mapping(
        &mut self,
        pipeline_id: PipelineId,
        from_node_id: NodeId,
        to_node_id: NodeId,
        output: &[usize],
    ) -> Result<bool> {
        let removed = self
            .pipeline_mut(pipeline_id)?
            .remove_mapping(from_node_id, to_node_id, output)
            .ok_or_else(|| format!("node {from_node_id} is not connected to {to_node_id}"))?;

        if removed {
            log_info!(
                "pipeline {pipeline_id}: {from_node_id} -> {to_node_id}, unmapped field {output:?}"
            );
        }

        Ok(removed)
    }

    /// Drops one mapping from a subscription's payload, leaving the rest alone.
    pub fn remove_subscription_mapping(
        &mut self,
        from: &str,
        event: usize,
        to: &str,
        command: usize,
        output: &[usize],
    ) -> Result<bool> {
        let handle = self.plugin(to)?.handle;

        let removed = event::manager()
            .remove_mapping(from, event, handle, command, output)
            .ok_or_else(|| {
                format!("'{to}' command {command} is not subscribed to event {event} of '{from}'")
            })?;

        if removed {
            log_info!(
                "'{from}' event {event} -> '{to}' command {command}: unmapped field {output:?}"
            );
        }

        Ok(removed)
    }

    pub fn start(&mut self, pipeline_id: PipelineId) -> Result<()> {
        let pipeline = self.pipeline_mut(pipeline_id)?;

        log_info!(
            "pipeline {pipeline_id} '{}': starting {} node(s)",
            pipeline.name,
            pipeline.nodes.len() + pipeline.workers.len()
        );

        pipeline.start();

        Ok(())
    }

    pub fn stop(&mut self, pipeline_id: PipelineId) -> Result<()> {
        let pipeline = self.pipeline_mut(pipeline_id)?;

        log_info!("pipeline {pipeline_id} '{}': stopping sources", pipeline.name);

        pipeline.stop();

        Ok(())
    }

    /// Nodes are named after their plugin, this renames one.
    pub fn set_node_name(
        &mut self,
        pipeline_id: PipelineId,
        node_id: NodeId,
        name: &str,
    ) -> Result<()> {
        if !self.pipeline_mut(pipeline_id)?.set_node_name(node_id, name) {
            return Err(format!("node {node_id} not found"));
        }

        log_info!("pipeline {pipeline_id}: node {node_id} renamed to '{name}'");

        Ok(())
    }

    pub fn set_node_running(
        &mut self,
        pipeline_id: PipelineId,
        node_id: NodeId,
        running: bool,
    ) -> Result<()> {
        if !self.pipeline_mut(pipeline_id)?.set_node_running(node_id, running) {
            return Err(format!("node {node_id} not started"));
        }

        log_info!(
            "pipeline {pipeline_id}: node {node_id} switched {}",
            if running { "on" } else { "off" }
        );

        Ok(())
    }

    pub fn send(&self, pipeline_id: PipelineId, node_id: NodeId, data: ProcessData) -> bool {
        let Some(pipeline) = self.pipelines.get(&pipeline_id) else {
            return false;
        };

        pipeline.send(node_id, data)
    }

    pub fn libraries(&self) -> Vec<(&str, &'static PluginDescriptor)> {
        self.descriptors
            .iter()
            .map(|(name, loaded)| (name.as_str(), loaded.descriptor))
            .collect()
    }

    /// Routes every pipeline's queued output.
    pub fn route_pending(&mut self) -> usize {
        self.pipelines
            .values_mut()
            .map(|pipeline| pipeline.route_pending())
            .sum()
    }

    /// Routes until the pipeline is quiet for `idle`, returns messages routed.
    pub async fn run(&mut self, pipeline_id: PipelineId, idle: Duration) -> Result<usize> {
        Ok(self.pipeline_mut(pipeline_id)?.run(idle).await)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Workers hold plugin handles, so they have to go first.
        self.pipelines.clear();

        // The event manager is static and outlives this app: a subscription
        // left behind would invoke a released plugin.
        let mut manager = event::manager();

        for plugin in self.plugins.values() {
            manager.forget(&plugin.id, plugin.handle);
            (plugin.descriptor.release)(plugin.handle);
        }
    }
}
