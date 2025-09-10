
#[derive(Debug, Clone)]
pub struct CommonType {
    pub name: String,
    pub id: i64,
}

impl CommonType {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            id: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapType {
    pub common: CommonType,
    pub key: i64,
    pub elem: i64,
}

#[derive(Debug, Clone)]
pub struct StructType {
    pub common: CommonType,
    pub fields: Vec<FieldType>,
}

#[derive(Debug, Clone)]
pub struct FieldType {
    pub name: String,
    pub id: i64,
}

#[derive(Debug, Clone)]
pub struct SliceType {
    pub common: CommonType,
    pub elem: i64,
}

#[derive(Debug, Clone)]
pub struct ArrayType {
    pub common: CommonType,
    pub elem: i64,
    pub len: i64,
}

#[derive(Debug, Clone)]
pub enum WireType {
    Array(ArrayType),
    Slice(SliceType),
    Struct(StructType),
    Map(MapType),
    GobEncoder(CommonType), // simplified
    BinaryMarshaler(CommonType),
    TextMarshaler(CommonType),
}

impl WireType {
    pub fn common(&self) -> &CommonType {
        match self {
            WireType::Array(t) => &t.common,
            WireType::Slice(t) => &t.common,
            WireType::Struct(t) => &t.common,
            WireType::Map(t) => &t.common,
            WireType::GobEncoder(t) => t,
            WireType::BinaryMarshaler(t) => t,
            WireType::TextMarshaler(t) => t,
        }
    }
}

