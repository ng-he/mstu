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

typedef struct Writer {
  /**
   * Reserved for engine use.
   * Plugins must never read or modify this field.
   */
  void *_engine_data;
  bool (*set_none)(struct Writer *w, uintptr_t field);
  bool (*set_bool)(struct Writer *w, uintptr_t field, bool value);
  bool (*set_int)(struct Writer *w, uintptr_t field, int64_t value);
  bool (*set_uint)(struct Writer *w, uintptr_t field, uint64_t value);
  bool (*set_float)(struct Writer *w, uintptr_t field, double value);
  bool (*set_str)(struct Writer *w, uintptr_t field, struct Str value);
  bool (*set_bytes)(struct Writer *w, uintptr_t field, struct Slice_u8 value);
  bool (*set_record)(struct Writer *w, uintptr_t field, struct Slice_Value value);
  bool (*set_list)(struct Writer *w, uintptr_t field, struct Slice_Value value);
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

typedef struct Message {
  struct Slice_Value values;
} Message;

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
