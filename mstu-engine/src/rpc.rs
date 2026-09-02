use std::{
    env,
    path::PathBuf,
    time::{Duration, Instant},
};

use mstu_sdk::{
    CommandDescriptor, EventDescriptor, Message, PluginDescriptor, Schema, Slice, Str, Type,
    TypeKind, Value,
};
use serde::Deserialize;
use serde_json::{Value as Json, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
};

use crate::{
    app::App,
    log_debug, log_info, log_trace, log_warn,
    runtime::{
        event::{self, Notice},
        mapper::{Mapper, Mapping},
        owned::Owned,
    },
};

/// How often queued pipeline output is routed while the socket is quiet.
const ROUTE_INTERVAL: Duration = Duration::from_millis(1);

/// How often plugin live snapshots are pushed to the client.
const LIVE_INTERVAL: Duration = Duration::from_millis(250);

pub fn socket_path() -> PathBuf {
    let dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(dir).join("mstu.sock")
}

#[derive(Deserialize)]
struct Request {
    id: u64,
    method: String,

    #[serde(default)]
    params: Json,
}

// ---------- describing plugins ----------

fn type_name(ty: &Type) -> &'static str {
    match ty.kind {
        TypeKind::TypeBool => "bool",
        TypeKind::TypeInt => {
            if unsafe { ty.schema.int_ }.signed {
                "int"
            } else {
                "uint"
            }
        }
        TypeKind::TypeFloat => "float",
        TypeKind::TypeString => "string",
        TypeKind::TypeRecord => "record",
        TypeKind::TypeEnum => "enum",
        TypeKind::TypeList => "list",
        TypeKind::TypeBytes => "bytes",
    }
}

fn schema_json(schema: *const Schema) -> Json {
    if schema.is_null() {
        return Json::Null;
    }

    let fields = unsafe { (*schema).fields.as_slice() };

    let fields: Vec<Json> = fields
        .iter()
        .map(|field| {
            json!({
                "name": unsafe { field.name.as_str() },
                "type": type_name(field.ty),
                "description": unsafe { field.description.as_str() },
            })
        })
        .collect();

    json!({ "fields": fields })
}

fn events_json(events: Slice<EventDescriptor>) -> Json {
    let events = unsafe { events.as_slice() };

    Json::Array(
        events
            .iter()
            .map(|event| {
                json!({
                    "name": unsafe { event.event_name.as_str() },
                    "schema": schema_json(event.schema),
                })
            })
            .collect(),
    )
}

fn commands_json(commands: Slice<CommandDescriptor>) -> Json {
    let commands = unsafe { commands.as_slice() };

    Json::Array(
        commands
            .iter()
            .map(|command| {
                json!({
                    "name": unsafe { command.command_name.as_str() },
                    "schema": schema_json(command.schema),
                })
            })
            .collect(),
    )
}

fn library_json(key: &str, descriptor: &'static PluginDescriptor) -> Json {
    let metadata = (descriptor.metadata)();

    json!({
        "key": key,
        "name": unsafe { metadata.name.as_str() },
        "version": unsafe { metadata.version.as_str() },
        "source": (descriptor.process_input_schema)().is_null(),
        "input": schema_json((descriptor.process_input_schema)()),
        "output": schema_json((descriptor.process_output_schema)()),
        "settings": schema_json((descriptor.settings_schema)()),
        "live": schema_json((descriptor.live_schema)()),
        "events": events_json((descriptor.events)()),
        "commands": commands_json((descriptor.commands)()),
    })
}

fn owned_json(value: &Owned) -> Json {
    match value {
        Owned::None => Json::Null,
        Owned::Bool(value) => json!(value),
        Owned::Int(value) => json!(value),
        Owned::Uint(value) => json!(value),
        Owned::Float(value) => json!(value),
        Owned::Str(value) => json!(value),
        Owned::Bytes(len) => json!({ "bytes": len }),
        Owned::List(values) => Json::Array(values.iter().map(owned_json).collect()),
    }
}

// ---------- reading params ----------

/// Owns the strings a `Value` borrows for the length of one call.
#[derive(Default)]
struct Storage {
    strings: Vec<String>,
}

impl Storage {
    fn value(&mut self, json: &Json) -> Result<Value, String> {
        Ok(match json {
            Json::Null => Value::none(),
            Json::Bool(value) => (*value).into(),
            Json::String(value) => {
                self.strings.push(value.clone());
                Str::new(self.strings.last().unwrap().as_str()).into()
            }
            Json::Number(value) => {
                if let Some(value) = value.as_u64() {
                    value.into()
                } else if let Some(value) = value.as_i64() {
                    value.into()
                } else {
                    value.as_f64().unwrap_or_default().into()
                }
            }
            _ => return Err(format!("cannot pass {json} across the ABI")),
        })
    }
}

fn field(params: &Json, name: &str) -> Result<Json, String> {
    params
        .get(name)
        .cloned()
        .ok_or_else(|| format!("missing '{name}'"))
}

fn as_str(params: &Json, name: &str) -> Result<String, String> {
    field(params, name)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("'{name}' must be a string"))
}

fn as_usize(params: &Json, name: &str) -> Result<usize, String> {
    field(params, name)?
        .as_u64()
        .map(|value| value as usize)
        .ok_or_else(|| format!("'{name}' must be a number"))
}

/// Mappings arrive as `[[from, to], ...]` field index pairs.
fn as_mapper(params: &Json, name: &str) -> Result<Mapper, String> {
    let mut mapper = Mapper::new();

    let Some(pairs) = params.get(name).and_then(Json::as_array) else {
        return Ok(mapper);
    };

    for pair in pairs {
        let pair = pair.as_array().ok_or("mapping must be [from, to]")?;
        let from = pair.first().and_then(Json::as_u64).ok_or("bad mapping")? as usize;
        let to = pair.get(1).and_then(Json::as_u64).ok_or("bad mapping")? as usize;

        mapper.add(Mapping {
            input: vec![from],
            output: vec![to],
        });
    }

    Ok(mapper)
}

// ---------- methods ----------

fn dispatch(app: &mut App, method: &str, params: &Json) -> Result<Json, String> {
    match method {
        "describe" => {
            let mut libraries: Vec<Json> = app
                .libraries()
                .into_iter()
                .map(|(key, descriptor)| library_json(key, descriptor))
                .collect();

            libraries.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));

            Ok(json!({ "libraries": libraries }))
        }

        "create_pipeline" => Ok(json!({
            "pipeline": app.create_pipeline(&as_str(params, "name")?)
        })),

        "create_plugin" => {
            let library = as_str(params, "library")?;
            let plugin = app.create_plugin(&library)?;
            let ui = app
                .plugin_ui(&plugin)?
                .map(|path| path.display().to_string());

            Ok(json!({ "plugin": plugin, "library": library, "ui": ui }))
        }

        "add_node" => Ok(json!({
            "node": app.add_node(as_usize(params, "pipeline")?, &as_str(params, "plugin")?)?
        })),

        "connect" => {
            app.connect(
                as_usize(params, "pipeline")?,
                as_usize(params, "from")?,
                as_usize(params, "to")?,
                as_mapper(params, "mappings")?,
            )?;

            Ok(json!({}))
        }

        "subscribe" => {
            app.subscribe(
                &as_str(params, "from")?,
                as_usize(params, "event")?,
                &as_str(params, "to")?,
                as_usize(params, "command")?,
                as_mapper(params, "mappings")?,
            )?;

            Ok(json!({}))
        }

        "unsubscribe" => {
            app.unsubscribe(
                &as_str(params, "from")?,
                as_usize(params, "event")?,
                &as_str(params, "to")?,
                as_usize(params, "command")?,
            )?;

            Ok(json!({}))
        }

        "set_parameter" => {
            let mut storage = Storage::default();
            let value = storage.value(&field(params, "value")?)?;

            app.set_parameter(&as_str(params, "plugin")?, as_usize(params, "field")?, value)?;

            Ok(json!({}))
        }

        "invoke" => {
            let mut storage = Storage::default();

            let payload = params
                .get("payload")
                .and_then(Json::as_array)
                .cloned()
                .unwrap_or_default();

            let values = payload
                .iter()
                .map(|item| storage.value(item))
                .collect::<Result<Vec<Value>, String>>()?;

            let input = Message {
                values: Slice::from(values.as_slice()),
            };

            app.invoke(
                &as_str(params, "plugin")?,
                as_usize(params, "command")?,
                &input,
            )?;

            Ok(json!({}))
        }

        "set_node_running" => {
            app.set_node_running(
                as_usize(params, "pipeline")?,
                as_usize(params, "node")?,
                field(params, "running")?.as_bool().unwrap_or(false),
            )?;

            Ok(json!({}))
        }

        "set_node_name" => {
            app.set_node_name(
                as_usize(params, "pipeline")?,
                as_usize(params, "node")?,
                &as_str(params, "name")?,
            )?;

            Ok(json!({}))
        }

        "start" => {
            app.start(as_usize(params, "pipeline")?)?;
            Ok(json!({}))
        }

        "stop" => {
            app.stop(as_usize(params, "pipeline")?)?;
            Ok(json!({}))
        }

        _ => Err(format!("unknown method '{method}'")),
    }
}

// ---------- serving ----------

/// One JSON object per line, both directions.
///
/// Requests get `{ id, ok, result | error }` back; published events arrive
/// unsolicited as `{ type: "event", plugin, event, values }`.
pub async fn serve(mut app: App) -> std::io::Result<()> {
    let path = socket_path();

    // Taking the path from a live engine would orphan its listener, leaving
    // it running but unreachable. Only a stale socket may be removed.
    if UnixStream::connect(&path).await.is_ok() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            format!("an engine is already serving on {}", path.display()),
        ));
    }

    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;

    log_info!("listening on {}", path.display());

    loop {
        // Keep routing while nobody is connected: the pipeline runs regardless.
        let stream = loop {
            tokio::select! {
                accepted = listener.accept() => break accepted?.0,
                _ = tokio::time::sleep(ROUTE_INTERVAL) => {
                    app.route_pending();
                }
            }
        };

        log_info!("client connected");

        serve_client(&mut app, stream).await;

        log_info!("client disconnected");
    }
}

async fn serve_client(app: &mut App, stream: UnixStream) {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();

    let (tap, mut notices) = unbounded_channel::<Notice>();
    event::set_tap(Some(tap));

    let mut polled = Instant::now();

    loop {
        let messages = tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => match respond(app, &line) {
                    Some(response) => vec![response],
                    None => continue,
                },
                // Client went away, wait for the next one.
                _ => break,
            },

            notice = notices.recv() => match notice {
                Some(notice) => vec![notice_json(&notice)],
                None => continue,
            },

            _ = tokio::time::sleep(ROUTE_INTERVAL) => {
                app.route_pending();

                if polled.elapsed() < LIVE_INTERVAL {
                    continue;
                }

                polled = Instant::now();
                live_json(app)
            }
        };

        let mut line = String::new();

        for message in &messages {
            line.push_str(&message.to_string());
            line.push('\n');
        }

        if line.is_empty() {
            continue;
        }

        if write.write_all(line.as_bytes()).await.is_err() {
            break;
        }
    }

    event::set_tap(None);
    drain(&mut notices);
}

fn respond(app: &mut App, line: &str) -> Option<Json> {
    if line.trim().is_empty() {
        return None;
    }

    Some(match serde_json::from_str::<Request>(line) {
        Ok(request) => {
            log_debug!("<- #{} {} {}", request.id, request.method, request.params);

            match dispatch(app, &request.method, &request.params) {
                Ok(result) => json!({ "id": request.id, "ok": true, "result": result }),
                Err(error) => {
                    log_warn!("#{} {} failed: {error}", request.id, request.method);

                    json!({ "id": request.id, "ok": false, "error": error })
                }
            }
        }
        Err(error) => {
            log_warn!("malformed request: {error}");

            json!({ "id": Json::Null, "ok": false, "error": error.to_string() })
        }
    })
}

/// One `live` line per plugin that publishes a snapshot.
fn live_json(app: &App) -> Vec<Json> {
    app.live_all()
        .into_iter()
        .map(|(plugin, values)| {
            json!({
                "type": "live",
                "plugin": plugin,
                "values": values.iter().map(owned_json).collect::<Vec<Json>>(),
            })
        })
        .collect()
}

fn notice_json(notice: &Notice) -> Json {
    log_trace!(
        "-> event {} of '{}'",
        notice.event,
        notice.plugin_id
    );

    json!({
        "type": "event",
        "plugin": notice.plugin_id,
        "event": notice.event,
        "values": notice.values.iter().map(owned_json).collect::<Vec<Json>>(),
    })
}

fn drain(notices: &mut UnboundedReceiver<Notice>) {
    while notices.try_recv().is_ok() {}
}
