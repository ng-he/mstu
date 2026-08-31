use crate::{Slice, Str, Value};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    TypeBool,
    TypeInt,
    TypeFloat,
    TypeString,
    TypeRecord,
    TypeEnum,
    TypeList,
    TypeBytes,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BoolSchema {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IntSchema {
    pub signed: bool,
    pub bits: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FloatSchema {
    pub bits: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StringSchema {}

#[repr(C)]
pub struct EnumVariant {
    pub name: Str,
    pub value: Value,
}

#[repr(C)]
pub struct EnumSchema {
    pub variants: Slice<EnumVariant>,
}

unsafe impl Sync for Slice<EnumVariant> {}
unsafe impl Send for Slice<EnumVariant> {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RecordSchema {
    pub schema: &'static Schema,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ListSchema {
    pub element: &'static Type,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BytesSchema {}

#[repr(C)]
pub union TypeSchema {
    pub bool_: BoolSchema,
    pub int_: IntSchema,
    pub float_: FloatSchema,
    pub string_: StringSchema,
    pub record_: RecordSchema,
    pub enum_: &'static EnumSchema,
    pub list_: ListSchema,
    pub bytes_: BytesSchema,
}

#[repr(C)]
pub struct Type {
    pub kind: TypeKind,
    pub schema: TypeSchema,
}

unsafe impl Sync for Type {}
unsafe impl Send for Type {}

#[repr(C)]
pub struct Field {
    pub name: Str,
    pub ty: &'static Type,
    pub description: Str,
}

unsafe impl Sync for Field {}
unsafe impl Send for Field {}

#[repr(C)]
pub struct Schema {
    pub fields: Slice<Field>,
}

unsafe impl Sync for Slice<Field> {}
unsafe impl Send for Slice<Field> {}

pub static TYPE_BOOL: Type = Type {
    kind: TypeKind::TypeBool,
    schema: TypeSchema {
        bool_: BoolSchema {},
    },
};

pub static TYPE_I8: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 8,
        },
    },
};

pub static TYPE_U8: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 8,
        },
    },
};

pub static TYPE_I16: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 16,
        },
    },
};

pub static TYPE_U16: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 16,
        },
    },
};

pub static TYPE_I32: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 32,
        },
    },
};

pub static TYPE_U32: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 32,
        },
    },
};

pub static TYPE_I64: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 64,
        },
    },
};

pub static TYPE_U64: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 64,
        },
    },
};

pub static TYPE_F32: Type = Type {
    kind: TypeKind::TypeFloat,
    schema: TypeSchema {
        float_: FloatSchema { bits: 32 },
    },
};

pub static TYPE_F64: Type = Type {
    kind: TypeKind::TypeFloat,
    schema: TypeSchema {
        float_: FloatSchema { bits: 64 },
    },
};

pub static TYPE_STRING: Type = Type {
    kind: TypeKind::TypeString,
    schema: TypeSchema {
        string_: StringSchema {},
    },
};

pub static TYPE_BYTES: Type = Type {
    kind: TypeKind::TypeBytes,
    schema: TypeSchema {
        bytes_: BytesSchema {},
    },
};
