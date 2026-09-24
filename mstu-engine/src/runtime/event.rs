use std::{
    collections::HashMap,
    ptr,
    sync::{LazyLock, Mutex, MutexGuard},
};

use mstu_sdk::{
    EventDescriptor, Message, PluginDescriptor, PluginHandle, Slice, Str, message,
};

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    log_debug, log_warn,
    runtime::{
        mapper::Mapper,
        owned::{Owned, own_message},
        process::{self, ProcessData},
        schema::{self, Shape},
    },
};

/// A published event, copied out for whoever is watching the engine.
pub struct Notice {
    pub plugin_id: String,
    pub event: usize,
    pub values: Vec<Owned>,
}

static TAP: Mutex<Option<UnboundedSender<Notice>>> = Mutex::new(None);

/// Mirrors every published event to `tap`, `None` stops it.
pub fn set_tap(tap: Option<UnboundedSender<Notice>>) {
    *TAP.lock().unwrap() = tap;
}

/// An event message, from `new_event` until it is published.
///
/// The queue boxes this: the writer handed to the plugin points at `data`,
/// so the address must stay put while the queue grows.
pub struct PendingEvent {
    pub event: usize,
    pub data: ProcessData,
    pub writer: message::Writer,
}

pub struct Subscription {
    pub mapper: Mapper,
    pub command: usize,
    pub plugin: PluginHandle,
    pub descriptor: &'static PluginDescriptor,

    /// Shape of the command payload, to build it and to check it.
    pub payload_shape: Vec<Shape>,
}

impl Subscription {
    pub fn new(
        plugin: PluginHandle,
        descriptor: &'static PluginDescriptor,
        command: usize,
        mapper: Mapper,
    ) -> Self {
        let commands = (descriptor.commands)();
        let payload = commands.get(command).map(|command| command.schema);

        Self {
            mapper,
            command,
            plugin,
            descriptor,
            payload_shape: payload.map(|schema| schema::shape_of(schema)).unwrap_or_default(),
        }
    }

    /// Maps the event payload into a command payload and invokes it.
    pub fn invoke(&self, event: &Message) -> bool {
        // Unmapped, so every field would arrive as none. A command that reads
        // its payload cannot survive that.
        if self.mapper.is_empty() && !self.payload_shape.is_empty() {
            log_warn!(
                "command {} skipped: it takes {} field(s) and nothing is mapped",
                self.command,
                self.payload_shape.len()
            );

            return false;
        }

        let mut data = ProcessData::from_shape(&self.payload_shape);

        if !self.mapper.map(event, &mut data.output, &mut data.arena) {
            log_warn!(
                "command {} skipped: mapping points at no such field",
                self.command
            );

            return false;
        }

        // A payload that does not match the command schema would crash it.
        if !schema::matches(&data.output, &self.payload_shape) {
            log_warn!(
                "command {} skipped: {} does not match its payload schema",
                self.command,
                data.output
            );

            return false;
        }

        log_debug!("command {} invoked with {:?}", self.command, data.output);

        (self.descriptor.invoke)(self.plugin, self.command, &data.output)
    }
}

pub struct Channel {
    pub events: Slice<EventDescriptor>,
    pub queue: Vec<Box<PendingEvent>>,

    /// Payload shape per event, indexed like `events`.
    pub shapes: Vec<Vec<Shape>>,

    /// Subscriptions per event, indexed like `events`.
    pub subscriptions: Vec<Vec<Subscription>>,
}

pub struct Manager {
    pub channels: HashMap<String, Channel>,
}

unsafe impl Send for Manager {}

impl Manager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Opens the channel a plugin emits its events on.
    pub fn register(&mut self, plugin_id: &str, descriptor: &'static PluginDescriptor) {
        let events = (descriptor.events)();
        let subscriptions = (0..events.len).map(|_| Vec::new()).collect();

        let shapes = unsafe { events.as_slice() }
            .iter()
            .map(|event| schema::shape_of(event.schema))
            .collect();

        self.channels.insert(
            plugin_id.to_string(),
            Channel {
                events,
                queue: Vec::new(),
                shapes,
                subscriptions,
            },
        );
    }

    pub fn subscribe(&mut self, plugin_id: &str, event: usize, subscription: Subscription) {
        let Some(channel) = self.channels.get_mut(plugin_id) else {
            return;
        };

        let Some(subscriptions) = channel.subscriptions.get_mut(event) else {
            return;
        };

        // Subscribing the same command on the same plugin again replaces its
        // mapping, the way reconnecting a pair does.
        match subscriptions.iter_mut().find(|existing| {
            existing.plugin == subscription.plugin && existing.command == subscription.command
        }) {
            Some(existing) => {
                log_debug!(
                    "'{plugin_id}' event {event}: replaced the subscription of command {}",
                    subscription.command
                );

                *existing = subscription;
            }
            None => {
                log_debug!(
                    "'{plugin_id}' event {event}: command {} subscribed",
                    subscription.command
                );

                subscriptions.push(subscription);
            }
        }
    }

    /// Drops the subscription of `command` on `plugin`, returns whether one
    /// was there.
    pub fn unsubscribe(
        &mut self,
        plugin_id: &str,
        event: usize,
        plugin: PluginHandle,
        command: usize,
    ) -> bool {
        let Some(channel) = self.channels.get_mut(plugin_id) else {
            return false;
        };

        let Some(subscriptions) = channel.subscriptions.get_mut(event) else {
            return false;
        };

        let before = subscriptions.len();

        subscriptions
            .retain(|existing| !(existing.plugin == plugin && existing.command == command));

        let removed = subscriptions.len() < before;

        if removed {
            log_debug!("'{plugin_id}' event {event}: command {command} unsubscribed");
        }

        removed
    }

    /// Drops the mapping filling `output` from the subscription of `command`.
    ///
    /// None when there is no such subscription, otherwise whether one was there.
    pub fn remove_mapping(
        &mut self,
        plugin_id: &str,
        event: usize,
        plugin: PluginHandle,
        command: usize,
        output: &[usize],
    ) -> Option<bool> {
        let subscription = self
            .channels
            .get_mut(plugin_id)?
            .subscriptions
            .get_mut(event)?
            .iter_mut()
            .find(|existing| existing.plugin == plugin && existing.command == command)?;

        Some(subscription.mapper.remove(output))
    }

    /// Closes a plugin's channel and drops every subscription pointing at it.
    ///
    /// A released plugin must not be reachable through an event it never saw
    /// unsubscribed.
    pub fn forget(&mut self, plugin_id: &str, plugin: PluginHandle) {
        self.channels.remove(plugin_id);

        for channel in self.channels.values_mut() {
            for subscriptions in channel.subscriptions.iter_mut() {
                subscriptions.retain(|existing| existing.plugin != plugin);
            }
        }
    }

    /// Queues an event and returns the writer that fills its payload.
    pub fn new_event(&mut self, plugin_id: &str, event: usize) -> *const message::Writer {
        let Some(channel) = self.channels.get_mut(plugin_id) else {
            return ptr::null();
        };

        let Some(shape) = channel.shapes.get(event) else {
            return ptr::null();
        };

        let mut pending = Box::new(PendingEvent {
            event,
            data: ProcessData::from_shape(shape),
            writer: process::new_writer(),
        });

        pending.writer._engine_data = (&mut pending.data as *mut ProcessData).cast();

        let writer: *const message::Writer = &pending.writer;
        channel.queue.push(pending);

        writer
    }

    /// Invokes every subscription of `event` with the queued payloads,
    /// then drops them.
    pub fn publish(&mut self, plugin_id: &str, event: usize) {
        let Some(channel) = self.channels.get_mut(plugin_id) else {
            return;
        };

        let mut publish = Vec::new();
        let mut keep = Vec::new();

        for pending in channel.queue.drain(..) {
            if pending.event == event {
                publish.push(pending);
            } else {
                keep.push(pending);
            }
        }

        channel.queue = keep;

        let Some(subscriptions) = channel.subscriptions.get(event) else {
            return;
        };

        log_debug!(
            "'{plugin_id}' published event {event}: {} message(s) to {} subscriber(s)",
            publish.len(),
            subscriptions.len()
        );

        for pending in &publish {
            for subscription in subscriptions {
                subscription.invoke(&pending.data.output);
            }
        }

        if let Some(tap) = TAP.lock().unwrap().as_ref() {
            for pending in &publish {
                let _ = tap.send(Notice {
                    plugin_id: plugin_id.to_string(),
                    event,
                    values: own_message(&pending.data.output),
                });
            }
        }
    }
}

static MANAGER: LazyLock<Mutex<Manager>> = LazyLock::new(|| Mutex::new(Manager::new()));

pub fn manager() -> MutexGuard<'static, Manager> {
    MANAGER.lock().unwrap()
}

/// `HostContext::new_event`.
pub extern "C" fn new_event(plugin_id: Str, event: usize) -> *const message::Writer {
    manager().new_event(unsafe { plugin_id.as_str() }, event)
}

/// `HostContext::publish_events`.
///
/// Holds the manager lock while subscribers run, so a plugin must not emit
/// events from a command handler.
pub extern "C" fn publish_events(plugin_id: Str, event: usize) {
    manager().publish(unsafe { plugin_id.as_str() }, event);
}
