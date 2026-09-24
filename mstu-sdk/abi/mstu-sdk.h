#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum ValueKind {
  ValueNone,
  ValueBool,
  ValueInt,
  ValueUint,
  ValueFloat,
  ValueString,
  ValueList,
  ValueRecord,
  ValueBytes,
} ValueKind;

typedef enum LogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
} LogLevel;

typedef enum TypeKind {
  TypeBool,
  TypeInt,
  TypeFloat,
  TypeString,
  TypeRecord,
  TypeEnum,
  TypeList,
  TypeBytes,
} TypeKind;

/**
 * A borrowed UTF-8 string.
 *
 * `Str` does not own the underlying memory. The caller must ensure that
 * `ptr` points to a valid UTF-8 buffer of length `len`, and that the buffer
 * remains alive for the entire time `Str` is used.
 *
 * This type is intended for FFI and is ABI-compatible with a pointer-length
 * string representation.
 */
typedef struct Str {
  const uint8_t *ptr;
  uintptr_t len;
} Str;

typedef struct Slice_Value {
  const struct Value *ptr;
  uintptr_t len;
} Slice_Value;

typedef struct Slice_u8 {
  const uint8_t *ptr;
  uintptr_t len;
} Slice_u8;

typedef union Data {
  uint64_t raw_;
  bool bool_;
  int64_t int_;
  uint64_t uint_;
  double float_;
  struct Str string_;
  struct Slice_Value list_;
  struct Slice_Value record_;
  struct Slice_u8 bytes_;
} Data;

typedef struct Value {
  enum ValueKind kind;
  union Data data;
} Value;

typedef struct Metadata {
  struct Str name;
  struct Str version;
} Metadata;

typedef void *PluginHandle;

typedef void (*LogFn)(enum LogLevel level, struct Str target, struct Str message);

typedef struct Message {
  struct Slice_Value values;
} Message;

/**
 * Fills in a message. The engine owns every byte one carries: a plugin
 * either lets `fill` copy what it has, or writes into `reserve`d memory.
 */
typedef struct Writer {
  /**
   * Reserved for engine use.
   * Plugins must never read or modify this field.
   */
  void *_engine_data;
  /**
   * Writes the whole message, one value per field, in order.
   *
   * What the values point at is copied in, except memory from `reserve`,
   * which the message already owns. False if the count is wrong.
   */
  bool (*fill)(struct Writer *w, struct Message message);
  /**
   * Engine memory for `len` bytes, living as long as the message.
   *
   * A value handed to `fill` may point at it, and nothing is copied.
   */
  uint8_t *(*reserve)(struct Writer *w, uintptr_t len);
} Writer;

typedef struct HostContext {
  /**
   * Host logger.
   */
  LogFn logger;
  /**
   * Whether the host would record a message at `level`.
   *
   * Lets a plugin skip formatting a message that would be dropped, so a
   * per-sample trace log costs nothing while trace is off.
   */
  bool (*log_enabled)(enum LogLevel level);
  /**
   * Creates a new pending event for the given plugin and event type.
   *
   * Returns a writer used to fill the event payload. The event remains
   * pending until `publish_event` is called.
   */
  const struct Writer *(*new_event)(struct Str plugin_id, uintptr_t event);
  /**
   * Publishes all pending events for the given plugin and event type.
   *
   * Pending events are published as a single batch and their temporary
   * resources may be reclaimed afterwards.
   */
  void (*publish_events)(struct Str plugin_id, uintptr_t event);
  /**
   * Publishes the plugin's live values, one per field of `live_schema`.
   *
   * The host copies what it needs before returning, so the snapshot may
   * point at anything the plugin owns. The latest one wins, so publishing
   * often is cheap: the host sends the UI what moved, when it moved.
   */
  void (*publish_live)(struct Str plugin_id, struct Message snapshot);
} HostContext;

typedef struct BoolSchema {

} BoolSchema;

typedef struct IntSchema {
  bool signed_;
  uint8_t bits;
} IntSchema;

typedef struct FloatSchema {
  uint8_t bits;
} FloatSchema;

typedef struct StringSchema {

} StringSchema;

typedef struct RecordSchema {
  const struct Schema *schema;
} RecordSchema;

typedef struct EnumVariant {
  struct Str name;
  struct Value value;
} EnumVariant;

typedef struct Slice_EnumVariant {
  const struct EnumVariant *ptr;
  uintptr_t len;
} Slice_EnumVariant;

typedef struct EnumSchema {
  struct Slice_EnumVariant variants;
} EnumSchema;

typedef struct ListSchema {
  const struct Type *element;
} ListSchema;

typedef struct BytesSchema {

} BytesSchema;

typedef union TypeSchema {
  struct BoolSchema bool_;
  struct IntSchema int_;
  struct FloatSchema float_;
  struct StringSchema string_;
  struct RecordSchema record_;
  const struct EnumSchema *enum_;
  struct ListSchema list_;
  struct BytesSchema bytes_;
} TypeSchema;

typedef struct Type {
  enum TypeKind kind;
  union TypeSchema schema;
} Type;

typedef struct Field {
  struct Str name;
  const struct Type *ty;
  struct Str description;
} Field;

typedef struct Slice_Field {
  const struct Field *ptr;
  uintptr_t len;
} Slice_Field;

typedef struct Schema {
  struct Slice_Field fields;
} Schema;

typedef struct ProcessContext {
  const struct Message *input;
  struct Writer *output;
} ProcessContext;

typedef struct EventDescriptor {
  /**
   * Event name.
   */
  struct Str event_name;
  /**
   * Payload schema.
   */
  const struct Schema *schema;
  struct Str description;
} EventDescriptor;

typedef struct Slice_EventDescriptor {
  const struct EventDescriptor *ptr;
  uintptr_t len;
} Slice_EventDescriptor;

typedef struct CommandDescriptor {
  /**
   * Command name.
   */
  struct Str command_name;
  /**
   * Payload schema.
   */
  const struct Schema *schema;
  struct Str description;
} CommandDescriptor;

typedef struct Slice_CommandDescriptor {
  const struct CommandDescriptor *ptr;
  uintptr_t len;
} Slice_CommandDescriptor;

typedef struct PluginDescriptor {
  /**
   * Gets plugins metadata
   */
  const struct Metadata *(*metadata)(void);
  /**
   * Creates a new plugin handle.
   */
  PluginHandle (*create)(const struct HostContext *ctx, struct Str id);
  /**
   * Get id of plugin
   */
  struct Str (*id)(PluginHandle handle);
  /**
   * Releases/deallocate a plugin.
   */
  void (*release)(PluginHandle handle);
  /**
   * Returns the process input schema.
   */
  const struct Schema *(*process_input_schema)(void);
  /**
   * Returns the process output schema.
   */
  const struct Schema *(*process_output_schema)(void);
  /**
   * Returns the settings schema.
   */
  const struct Schema *(*settings_schema)(void);
  /**
   * Gets the current settings.
   */
  bool (*settings)(PluginHandle handle, struct Writer *output);
  /**
   * Updates a single setting.
   */
  bool (*set_parameter)(PluginHandle handle, uintptr_t field, struct Value value);
  /**
   * Returns the plugin UI entry.
   */
  struct Str (*ui)(void);
  /**
   * Process a stream.
   */
  bool (*process)(PluginHandle handle, struct ProcessContext *p_ctx);
  /**
   * Returns events.
   */
  struct Slice_EventDescriptor (*events)(void);
  /**
   * Returns commands.
   */
  struct Slice_CommandDescriptor (*commands)(void);
  /**
   * Invokes a command.
   */
  bool (*invoke)(PluginHandle handle, uintptr_t command, const struct Message *input);
  /**
   * Returns the schema of the live values the UI shows, null when there are none.
   *
   * The plugin pushes them with `HostContext::publish_live`.
   */
  const struct Schema *(*live_schema)(void);
} PluginDescriptor;

const struct Value *value_get(struct Slice_Value slice, uintptr_t index);

/**
 * Dummy function to force `Value` to be emitted before dependent types in the
 * generated C header. This works around a cbindgen declaration-ordering issue.
 */
void __cbindgen_force_value(struct Value _v);

/**
 * Returns the plugin descriptor exported by the shared library.
 */
extern const struct PluginDescriptor *plugin_descriptor(void);

struct Str str_from_cstr(const char *ptr);

static inline struct Value mstu_none(void) {
  struct Value v = { ValueNone, { .uint_ = 0 } };
  return v;
}

static inline struct Value mstu_bool(bool value) {
  struct Value v = { ValueBool, { .bool_ = value } };
  return v;
}

static inline struct Value mstu_int(int64_t value) {
  struct Value v = { ValueInt, { .int_ = value } };
  return v;
}

static inline struct Value mstu_uint(uint64_t value) {
  struct Value v = { ValueUint, { .uint_ = value } };
  return v;
}

static inline struct Value mstu_float(double value) {
  struct Value v = { ValueFloat, { .float_ = value } };
  return v;
}

static inline struct Value mstu_str(const char *text, uintptr_t len) {
  struct Value v = { ValueString, { .string_ = { (const uint8_t *)text, len } } };
  return v;
}

static inline struct Value mstu_bytes(const uint8_t *ptr, uintptr_t len) {
  struct Value v = { ValueBytes, { .bytes_ = { ptr, len } } };
  return v;
}

static inline struct Value mstu_list(const struct Value *values, uintptr_t len) {
  struct Value v = { ValueList, { .list_ = { values, len } } };
  return v;
}

static inline struct Value mstu_record(const struct Value *fields, uintptr_t len) {
  struct Value v = { ValueRecord, { .record_ = { fields, len } } };
  return v;
}

/// Writes the whole message: one value per field, in order.
static inline bool mstu_fill(struct Writer *w, const struct Value *values, uintptr_t len) {
  struct Message message = { { values, len } };
  return w->fill(w, message);
}

/// Engine memory for a payload, written in place and never copied.
static inline uint8_t *mstu_reserve(struct Writer *w, uintptr_t len) {
  return w->reserve(w, len);
}

/// Publishes the live values the UI shows: one per field of the live schema.
static inline void mstu_publish_live(const struct HostContext *ctx,
                                     struct Str plugin_id,
                                     const struct Value *values,
                                     uintptr_t len) {
  struct Message message = { { values, len } };
  ctx->publish_live(plugin_id, message);
}
