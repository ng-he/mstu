use mstu_sdk::{
    CommandDescriptor, Data, EnumSchema, EnumVariant, Field, Schema, Slice, Str, TYPE_BYTES,
    TYPE_STRING, Type, TypeKind::TypeEnum, TypeSchema, Value, ValueKind, slice,
};

pub static SETTINGS_SCHEMA_FIELDS: [Field; 1] = [Field {
    name: Str::from_static("output_path"),
    ty: &TYPE_STRING,
    description: Str::from_static("Path to the output file."),
}];

pub static SETTINGS_SCHEMA: Schema = Schema {
    fields: slice!(SETTINGS_SCHEMA_FIELDS),
};

pub static INPUT_SCHEMA_FIELDS: [Field; 1] = [Field {
    name: Str::from_static("data"),
    ty: &TYPE_BYTES,
    description: Str::from_static("Encoded media data to write."),
}];

pub static INPUT_SCHEMA: Schema = Schema {
    fields: slice!(INPUT_SCHEMA_FIELDS),
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

pub static CHANGE_CODEC_COMMAND_SCHEMA_FIELDS: [Field; 2] = [
    Field {
        name: Str::from_static("codec"),
        ty: &CODEC_ENUM_TYPE,
        description: Str::from_static("Target codec for the output stream."),
    },
    Field {
        name: Str::from_static("extras"),
        ty: &TYPE_BYTES,
        description: Str::from_static("Codec-specific configuration data."),
    },
];

pub static CHANGE_CODEC_COMMAND_SCHEMA: Schema = Schema {
    fields: slice!(CHANGE_CODEC_COMMAND_SCHEMA_FIELDS),
};

pub static FINISH_WRITE_COMMAND_SCHEMA: Schema = Schema {
    fields: Slice::empty(),
};

pub static COMMANDS: [CommandDescriptor; 2] = [
    CommandDescriptor {
        command_name: Str::from_static("codec.change"),
        schema: &CHANGE_CODEC_COMMAND_SCHEMA,
        description: Str::from_static("Changes the codec of the output stream."),
    },
    CommandDescriptor {
        command_name: Str::from_static("write.finish"),
        schema: &FINISH_WRITE_COMMAND_SCHEMA,
        description: Str::from_static("Finishes writing and closes the current output."),
    },
];
