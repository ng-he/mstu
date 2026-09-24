use std::matches;

use mstu_sdk::{Message, Value, ValueKind};

use crate::runtime::{alloc::Arena, schema::Shape, value::clone_value};

/// Deepest path the engine describes, so a path never needs the heap.
pub const MAX_PATH: usize = 8;

/// A path to a field, held inline: one or two steps is the normal case, and a
/// heap read per mapping per message is not worth paying for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Path {
    steps: [u32; MAX_PATH],
    len: u8,
}

impl Path {
    pub fn new(steps: &[usize]) -> Option<Self> {
        if steps.is_empty() || steps.len() > MAX_PATH {
            return None;
        }

        let mut path = Self {
            steps: [0; MAX_PATH],
            len: steps.len() as u8,
        };

        for (slot, step) in path.steps.iter_mut().zip(steps) {
            *slot = u32::try_from(*step).ok()?;
        }

        Some(path)
    }

    #[inline]
    pub fn as_slice(&self) -> &[u32] {
        &self.steps[..self.len as usize]
    }

    /// The field index when the path is a plain one, which nearly all are.
    #[inline]
    fn flat(&self) -> Option<usize> {
        (self.len == 1).then_some(self.steps[0] as usize)
    }
}

/// One mapping resolved ahead of time, so running it reads no `Vec`.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// Top-level field to top-level field: an index either side.
    Flat {
        from: usize,
        to: usize,
    },
    Nested {
        from: Path,
        to: Path,
    },
}

#[derive(Debug, Clone)]
pub struct Mapping {
    /// Path to the source value in the input message.
    ///
    /// Example: [2, 4, 1] means:
    /// input[2].record[4].record[1]
    pub input: Vec<usize>,

    /// Path to the destination value in the output message.
    ///
    /// Example: [0, 3, 2] means:
    /// output[0].record[3].record[2]
    pub output: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct Mapper {
    mappings: Vec<Mapping>,

    /// The mappings compiled down to index pairs, rebuilt whenever they
    /// change rather than walked per message.
    plan: Vec<Step>,

    /// Whether the plan is "field 0 to field 0, field 1 to field 1, …", in
    /// which case the values go across as they are.
    identity: bool,
}

impl Mapper {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, mapping: Mapping) {
        self.mappings.push(mapping);
        self.compile();
    }

    /// Drops whatever fills `output`, and says whether anything did.
    ///
    /// A field is filled by one mapping, so its path names the mapping.
    pub fn remove(&mut self, output: &[usize]) -> bool {
        let before = self.mappings.len();

        self.mappings
            .retain(|mapping| mapping.output.as_slice() != output);

        let removed = self.mappings.len() < before;

        if removed {
            self.compile();
        }

        removed
    }

    /// Resolves every mapping into a step the routing loop can run blind.
    fn compile(&mut self) {
        self.plan = self
            .mappings
            .iter()
            .filter_map(|mapping| {
                let from = Path::new(&mapping.input)?;
                let to = Path::new(&mapping.output)?;

                Some(match (from.flat(), to.flat()) {
                    (Some(from), Some(to)) => Step::Flat { from, to },
                    _ => Step::Nested { from, to },
                })
            })
            .collect();

        self.identity = self.plan.iter().enumerate().all(|(index, step)| {
            matches!(step, Step::Flat { from, to } if *from == index && *to == index)
        });
    }

    /// Whether this mapping fills every field of `shape` from `source`, with
    /// the kinds lining up, so a mapped message needs no checking afterwards.
    pub fn covers(&self, source: &[Shape], target: &[Shape]) -> bool {
        if self.plan.len() != target.len() {
            return false;
        }

        target.iter().enumerate().all(|(index, want)| {
            self.plan.iter().any(|step| match step {
                Step::Flat { from, to } => {
                    *to == index && source.get(*from).is_some_and(|had| had.kind == want.kind)
                }

                // A nested field is checked per message: its record may be
                // replaced wholesale by whatever the source carries.
                Step::Nested { .. } => false,
            })
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mapping> {
        self.mappings.iter()
    }

    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Copies the mapped fields across, buffers and all.
    ///
    /// For a message that outlives its source — an event payload handed to a
    /// command — because the values cannot point back at something released.
    pub fn map(&self, input: &Message, output: &mut Message, arena: &mut Arena) -> bool {
        self.run(input, output, |value, dst| {
            *dst = unsafe { clone_value(arena, value) };
        })
    }

    /// Points the mapped fields at the source instead of copying them.
    ///
    /// A payload travels as a pointer, however big it is. Whoever calls this
    /// keeps the source alive: see `ProcessData::borrows_from`.
    pub fn share(&self, input: &Message, output: &mut Message) -> bool {
        // Same fields, same order: the whole row goes across in one copy.
        if self.identity && input.values_slice().len() == output.values_slice().len() {
            output
                .values_slice_mut()
                .copy_from_slice(input.values_slice());

            return true;
        }

        self.run(input, output, |value, dst| *dst = *value)
    }

    fn run(
        &self,
        input: &Message,
        output: &mut Message,
        mut carry: impl FnMut(&Value, &mut Value),
    ) -> bool {
        for step in &self.plan {
            match step {
                Step::Flat { from, to } => {
                    let Some(value) = input.values_slice().get(*from) else {
                        return false;
                    };

                    let value = *value;

                    let Some(dst) = output.values_slice_mut().get_mut(*to) else {
                        return false;
                    };

                    carry(&value, dst);
                }

                Step::Nested { from, to } => {
                    let Some(value) = at(input, from.as_slice()) else {
                        return false;
                    };

                    let value = *value;

                    let Some(dst) = at_mut(output, to.as_slice()) else {
                        return false;
                    };

                    carry(&value, dst);
                }
            }
        }

        true
    }
}

/// Walks a path into a message.
///
/// An empty path means the root message, which is not a value, so this
/// returns None.
///
/// Example:
///
/// [2]       -> message.values[2]
/// [2, 4]    -> message.values[2].record[4]
/// [2, 4, 1] -> message.values[2].record[4].record[1]
fn at<'a>(message: &'a Message, path: &[u32]) -> Option<&'a Value> {
    let (first, rest) = path.split_first()?;
    let mut value = message.values_slice().get(*first as usize)?;

    for index in rest {
        value = get_record_value(value, *index as usize)?;
    }

    Some(value)
}

/// Mutable version of `at`.
fn at_mut<'a>(message: &'a mut Message, path: &[u32]) -> Option<&'a mut Value> {
    let (first, rest) = path.split_first()?;
    let mut value = message.values_slice_mut().get_mut(*first as usize)?;

    for index in rest {
        value = get_record_value_mut(value, *index as usize)?;
    }

    Some(value)
}

fn get_record_value<'a>(value: &'a Value, index: usize) -> Option<&'a Value> {
    if value.kind != ValueKind::ValueRecord {
        return None;
    }

    unsafe { value.data.record_.as_slice().get(index) }
}

fn get_record_value_mut<'a>(value: &'a mut Value, index: usize) -> Option<&'a mut Value> {
    if value.kind != ValueKind::ValueRecord {
        return None;
    }

    unsafe {
        let record = value.data.record_;
        std::slice::from_raw_parts_mut(record.ptr as *mut Value, record.len).get_mut(index)
    }
}

#[cfg(test)]
mod tests {
    use mstu_sdk::{
        Field, RecordSchema, Schema, Slice, Str, TYPE_BYTES, TYPE_STRING, TYPE_U64, Type, TypeKind,
        TypeSchema, slice,
    };

    use super::*;
    use crate::runtime::{process::ProcessData, schema};

    // { "id": u64, "codec": { "name": string, "bytes": bytes } }
    static INNER_FIELDS: [Field; 2] = [
        Field {
            name: Str::from_static("name"),
            ty: &TYPE_STRING,
            description: Str::empty(),
        },
        Field {
            name: Str::from_static("bytes"),
            ty: &TYPE_BYTES,
            description: Str::empty(),
        },
    ];

    static INNER: Schema = Schema {
        fields: slice!(INNER_FIELDS),
    };

    static INNER_TYPE: Type = Type {
        kind: TypeKind::TypeRecord,
        schema: TypeSchema {
            record_: RecordSchema { schema: &INNER },
        },
    };

    static OUTER_FIELDS: [Field; 2] = [
        Field {
            name: Str::from_static("id"),
            ty: &TYPE_U64,
            description: Str::empty(),
        },
        Field {
            name: Str::from_static("codec"),
            ty: &INNER_TYPE,
            description: Str::empty(),
        },
    ];

    static OUTER: Schema = Schema {
        fields: slice!(OUTER_FIELDS),
    };

    static FLAT_FIELDS: [Field; 1] = [Field {
        name: Str::from_static("name"),
        ty: &TYPE_STRING,
        description: Str::empty(),
    }];

    static FLAT: Schema = Schema {
        fields: slice!(FLAT_FIELDS),
    };

    /// A record field is built out, so a mapping has somewhere to land.
    #[test]
    fn builds_nested_records() {
        let data = ProcessData::from_shape(&schema::shape_of(&OUTER));
        let values = data.output.values_slice();

        assert_eq!(values.len(), 2);
        assert!(values[0].kind == ValueKind::ValueNone);
        assert!(values[1].kind == ValueKind::ValueRecord);
        assert_eq!(unsafe { values[1].data.record_.as_slice() }.len(), 2);

        // Nothing written yet, however many records were laid out.
        assert!(data.is_empty());
    }

    /// The whole point of paths: source field -> field inside a record.
    #[test]
    fn maps_into_a_nested_field() {
        let mut source = ProcessData::from_shape(&schema::shape_of(&FLAT));
        let mut target = ProcessData::from_shape(&schema::shape_of(&OUTER));

        let name = Str::from_static("h264");
        source.output.values_slice_mut()[0] = name.into();

        let mut mapper = Mapper::new();
        mapper.add(Mapping {
            input: vec![0],
            output: vec![1, 0],
        });

        assert!(mapper.map(&source.output, &mut target.output, &mut target.arena));
        assert!(!target.is_empty());

        let landed = unsafe { target.output.values_slice()[1].data.record_.as_slice() }[0];

        assert!(landed.kind == ValueKind::ValueString);
        assert_eq!(unsafe { landed.data.string_.as_str() }, "h264");

        // The copy is the engine's own, not the source's buffer.
        assert_ne!(unsafe { landed.data.string_.ptr }, name.ptr);
    }

    // { "id": u64, "data": bytes }: the shape a media message really has.
    static FRAME_FIELDS: [Field; 2] = [
        Field {
            name: Str::from_static("id"),
            ty: &TYPE_U64,
            description: Str::empty(),
        },
        Field {
            name: Str::from_static("data"),
            ty: &TYPE_BYTES,
            description: Str::empty(),
        },
    ];

    static FRAME: Schema = Schema {
        fields: slice!(FRAME_FIELDS),
    };

    /// Builds a message with a payload of `size` bytes in its second field.
    fn frame(size: usize) -> ProcessData {
        let mut data = ProcessData::from_shape(&schema::shape_of(&FRAME));
        let buffer = data.arena.alloc::<u8>(size);

        unsafe { std::ptr::write_bytes(buffer, 0xAB, size) };

        data.output.values_slice_mut()[0] = 1u64.into();
        data.output.values_slice_mut()[1] = Slice::from_raw_parts(buffer, size).into();

        data
    }

    fn both_fields() -> Mapper {
        let mut mapper = Mapper::new();

        mapper.add(Mapping {
            input: vec![0],
            output: vec![0],
        });

        mapper.add(Mapping {
            input: vec![1],
            output: vec![1],
        });

        mapper
    }

    /// The payload travels as a pointer: same bytes, same address.
    #[test]
    fn shares_the_payload_instead_of_copying_it() {
        let source = frame(64 * 1024);
        let mut target = ProcessData::from_shape(&schema::shape_of(&FRAME));

        assert!(both_fields().share(&source.output, &mut target.output));

        let from = unsafe { source.output.values_slice()[1].data.bytes_ };
        let to = unsafe { target.output.values_slice()[1].data.bytes_ };

        assert_eq!(from.ptr, to.ptr);
        assert_eq!(from.len, to.len);

        // And copying, for comparison, does not.
        let mut copied = ProcessData::from_shape(&schema::shape_of(&FRAME));
        let mut arena = crate::runtime::alloc::acquire();

        assert!(both_fields().map(&source.output, &mut copied.output, &mut arena));

        let duplicate = unsafe { copied.output.values_slice()[1].data.bytes_ };

        assert_ne!(duplicate.ptr, from.ptr);
        assert_eq!(unsafe { duplicate.as_slice() }, unsafe { from.as_slice() });

        crate::runtime::alloc::release(arena);
    }

    /// Field for field in order: the whole row goes across in one copy.
    #[test]
    fn recognises_an_identity_mapping() {
        let mapper = both_fields();
        assert!(mapper.identity);

        let mut crossed = Mapper::new();

        crossed.add(Mapping {
            input: vec![1],
            output: vec![0],
        });

        assert!(!crossed.identity);
    }

    /// A mapping that fills every field with the right kind needs no checking
    /// per message; one that leaves a field empty does.
    #[test]
    fn knows_when_a_message_needs_no_checking() {
        let shape = schema::shape_of(&FRAME);

        assert!(both_fields().covers(&shape, &shape));

        let mut partial = Mapper::new();

        partial.add(Mapping {
            input: vec![0],
            output: vec![0],
        });

        assert!(!partial.covers(&shape, &shape));

        // Right fields, wrong types: bytes into a u64.
        let mut crossed = Mapper::new();

        crossed.add(Mapping {
            input: vec![1],
            output: vec![0],
        });

        crossed.add(Mapping {
            input: vec![0],
            output: vec![1],
        });

        assert!(!crossed.covers(&shape, &shape));
    }

    /// What the sharing is for: a frame-sized payload, both ways.
    #[test]
    fn sharing_beats_copying_on_a_frame() {
        const SIZE: usize = 384 * 1024;
        const ROUNDS: usize = 200;

        let source = frame(SIZE);
        let mapper = both_fields();

        let copying = std::time::Instant::now();

        for _ in 0..ROUNDS {
            let mut target = ProcessData::from_shape(&schema::shape_of(&FRAME));
            let mut arena = crate::runtime::alloc::acquire();

            assert!(mapper.map(&source.output, &mut target.output, &mut arena));

            crate::runtime::alloc::release(arena);
        }

        let copying = copying.elapsed();
        let sharing = std::time::Instant::now();

        for _ in 0..ROUNDS {
            let mut target = ProcessData::from_shape(&schema::shape_of(&FRAME));
            assert!(mapper.share(&source.output, &mut target.output));
        }

        let sharing = sharing.elapsed();

        println!(
            "{SIZE} byte payload x{ROUNDS}: copying {copying:?} ({:.0} MB/s), sharing {sharing:?}",
            (SIZE * ROUNDS) as f64 / copying.as_secs_f64() / 1e6
        );

        // Not a close call: one moves the bytes, the other moves a pointer.
        assert!(sharing * 4 < copying);
    }

    /// A record that came out the wrong shape must not reach a plugin.
    #[test]
    fn checks_a_record_field_by_field() {
        static DATA: [u8; 3] = [1, 2, 3];

        let shape = schema::shape_of(&OUTER);
        let mut data = ProcessData::from_shape(&shape);

        // Every field filled, inside the record as well: an unfilled field is
        // a field the target would unwrap as none.
        data.output.values_slice_mut()[0] = 7u64.into();

        let inner = unsafe { data.output.values_slice()[1].data.record_ };
        let held = unsafe { std::slice::from_raw_parts_mut(inner.ptr as *mut Value, inner.len) };

        held[0] = Str::from_static("h264").into();
        held[1] = slice!(DATA).into();

        assert!(schema::matches(&data.output, &shape));

        // One field inside the record given the wrong kind.
        held[1] = 9u64.into();
        assert!(!schema::matches(&data.output, &shape));

        held[1] = slice!(DATA).into();
        assert!(schema::matches(&data.output, &shape));

        // A list where a record belongs: same slice, wrong kind.
        data.output.values_slice_mut()[1] = Value::list(inner);
        assert!(!schema::matches(&data.output, &shape));
    }
}
