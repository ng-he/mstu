use std::ptr;

use mstu_sdk::{
    Data, EnumSchema, EnumVariant, EventDescriptor, Field, Schema, Slice, Str, TYPE_BOOL,
    TYPE_BYTES, TYPE_F64, TYPE_I32, TYPE_STRING, TYPE_U32, TYPE_U64, Type, TypeKind::TypeEnum,
    TypeSchema,
    Value, ValueKind, slice,
};

pub static SETTINGS_SCHEMA_FIELDS: [Field; 4] = [
    Field {
        name: Str::from_static("path"),
        ty: &TYPE_STRING,
        description: Str::from_static("Path to the media file."),
    },
    Field {
        name: Str::from_static("rate"),
        ty: &TYPE_F64,
        description: Str::from_static(
            "Playback speed: 1.0 is real time, 2.0 twice as fast, 0 reads as fast as it can.",
        ),
    },
    Field {
        name: Str::from_static("loop"),
        ty: &TYPE_BOOL,
        description: Str::from_static("Start over at the end of the file instead of stopping."),
    },
    Field {
        name: Str::from_static("position"),
        ty: &TYPE_U64,
        description: Str::from_static("Seeks to this position in microseconds."),
    },
];

pub static SETTINGS_SCHEMA: Schema = Schema {
    fields: slice!(SETTINGS_SCHEMA_FIELDS),
};

pub static LIVE_SCHEMA_FIELDS: [Field; 4] = [
    Field {
        name: Str::from_static("samples"),
        ty: &TYPE_U64,
        description: Str::from_static("Samples read since the file was opened."),
    },
    Field {
        name: Str::from_static("ended"),
        ty: &TYPE_BOOL,
        description: Str::from_static("Whether the reader has reached the end of the file."),
    },
    Field {
        name: Str::from_static("position"),
        ty: &TYPE_U64,
        description: Str::from_static("Time of the last sample read, in microseconds."),
    },
    Field {
        name: Str::from_static("duration"),
        ty: &TYPE_U64,
        description: Str::from_static("Length of the file in microseconds, 0 when unknown."),
    },
];

pub static LIVE_SCHEMA: Schema = Schema {
    fields: slice!(LIVE_SCHEMA_FIELDS),
};

pub static VIDEO_OUTPUT_SCHEMA_FIELDS: [Field; 5] = [
    Field {
        name: Str::from_static("start_time"),
        ty: &TYPE_U64,
        description: Str::from_static("Start time of the sample."),
    },
    Field {
        name: Str::from_static("duration"),
        ty: &TYPE_U32,
        description: Str::from_static("Duration of the sample."),
    },
    Field {
        name: Str::from_static("rendering_offset"),
        ty: &TYPE_I32,
        description: Str::from_static("Rendering offset of the sample."),
    },
    Field {
        name: Str::from_static("is_sync"),
        ty: &TYPE_BOOL,
        description: Str::from_static("Whether the sample is a synchronization point."),
    },
    Field {
        name: Str::from_static("bytes"),
        ty: &TYPE_BYTES,
        description: Str::from_static(
            "Encoded video sample data, Annex-B start code prefixed for h26x.",
        ),
    },
];

pub static VIDEO_OUTPUT_SCHEMA: Schema = Schema {
    fields: slice!(VIDEO_OUTPUT_SCHEMA_FIELDS),
};

pub static CODEC_ENUM_VARIANTS: [EnumVariant; 2] = [
    EnumVariant {
        name: Str::from_static("H264"),
        value: Value {
            kind: ValueKind::ValueString,
            data: Data {
                string_: Str::from_static("h264"),
            },
        },
    },
    EnumVariant {
        name: Str::from_static("H265"),
        value: Value {
            kind: ValueKind::ValueString,
            data: Data {
                string_: Str::from_static("h265"),
            },
        },
    },
];

pub static CODEC_ENUM_SCHEMA: EnumSchema = EnumSchema {
    variants: slice!(CODEC_ENUM_VARIANTS),
};

pub static CODEC_ENUM_TYPE: Type = Type {
    kind: TypeEnum,
    schema: TypeSchema {
        enum_: &CODEC_ENUM_SCHEMA,
    },
};

pub static FILE_CHANGED_SCHEMA_FIELDS: [Field; 3] = [
    Field {
        name: Str::from_static("file_name"),
        ty: &TYPE_STRING,
        description: Str::from_static("Name of the newly opened output file."),
    },
    Field {
        name: Str::from_static("codec"),
        ty: &CODEC_ENUM_TYPE,
        description: Str::from_static("Codec used by the output stream."),
    },
    Field {
        name: Str::from_static("extras"),
        ty: &TYPE_BYTES,
        description: Str::from_static(
            "Codec-specific configuration data, Annex-B start code prefixed for h26x.",
        ),
    },
];

pub static FILE_CHANGED_SCHEMA: Schema = Schema {
    fields: slice!(FILE_CHANGED_SCHEMA_FIELDS),
};

pub static FILE_ENDED_SCHEMA: Schema = Schema {
    fields: Slice::empty(),
};

pub static EVENTS: [EventDescriptor; 2] = [
    EventDescriptor {
        event_name: Str::from_static("file.changed"),
        schema: &FILE_CHANGED_SCHEMA,
        description: Str::from_static("Emitted when a new output file is opened."),
    },
    EventDescriptor {
        event_name: Str::from_static("file.ended"),
        schema: &FILE_ENDED_SCHEMA,
        description: Str::from_static("Emitted when the current file reaches the end."),
    },
];
