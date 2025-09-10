use std::collections::BTreeMap;
use std::cmp::Ordering;
use serde::{Serialize, Deserialize};
use crate::{Encoder, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    String(String),
    #[serde(with = "serde_bytes")]
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    // Using BTreeMap for consistent ordering and Hash/Eq requirements
    Map(BTreeMap<Value, Value>), 
    Struct(String, BTreeMap<String, Value>), // Name, Fields
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl TryFrom<Value> for String {
    type Error = std::io::Error;
    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::String(s) => Ok(s),
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Expected String, got {:?}", v))),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = std::io::Error;
    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Int(i) => Ok(i),
            Value::Uint(u) => Ok(u as i64), // Loose conversion
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Expected Int, got {:?}", v))),
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = std::io::Error;
    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Bool(b) => Ok(b),
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Expected Bool, got {:?}", v))),
        }
    }
}

impl Into<Value> for i64 {
    fn into(self) -> Value {
        Value::Int(self)
    }
}

impl Into<Value> for u64 {
    fn into(self) -> Value {
        Value::Uint(self)
    }
}
impl Into<Value> for f64 {
    fn into(self) -> Value {
        Value::Float(self)
    }
}

impl Into<Value> for Vec<u8> {
    fn into(self) -> Value {
        Value::Bytes(self)
    }
}


// Type alias for map[interface{}]interface{}
pub type GobMap = BTreeMap<Value, Value>;

impl Value {
    pub fn encode<W: std::io::Write>(&self, encoder: &mut Encoder<W>) -> Result<()> {
         // This is a naive implementation that just encodes the value itself.
         // In real Gob, we need to transmit Type Definitions (WireTypes) first if they are new.
         // And we need to include TypeIDs.
         
         // For now, let's implement basic encoding of the value content.
         // This is useful for "inner" values or simple testing.
         // BUT standard Gob format requires a message structure: [Length] [TypeID] [Value].
         
         match self {
             Value::Nil => {
                 // Nil is usually not encoded directly on wire as a value, but as an empty interface or skipped field?
                 // Or maybe 0 value?
                 Ok(())
             }
             Value::Bool(v) => encoder.write_bool(*v),
             Value::Int(v) => encoder.write_int(*v),
             Value::Uint(v) => encoder.write_uint(*v),
             Value::Float(v) => encoder.write_float(*v),
             Value::String(v) => encoder.write_string(v),
             Value::Bytes(v) => encoder.write_bytes(v),
             Value::Array(v) => {
                 encoder.write_uint(v.len() as u64)?;
                 for item in v {
                     item.encode(encoder)?;
                 }
                 Ok(())
             }
             Value::Map(m) => {
                 encoder.write_uint(m.len() as u64)?;
                 for (k, v) in m {
                     k.encode(encoder)?;
                     v.encode(encoder)?;
                 }
                 Ok(())
             }
             Value::Struct(_name, fields) => {
                 // Structs in Gob are delta-encoded.
                 // We need to know the field numbers from the schema.
                 // Without schema, we can't properly encode a struct that a standard Gob decoder would understand
                 // (unless we also define and send the WireType).
                 
                 // For this exercise, maybe we assume a specific schema or just write fields?
                 // But writing fields without field numbers is invalid Gob struct data.
                 
                 // Let's just iterate and assume field numbers increment (1, 2, 3...)?
                 // Or maybe we just skip implementation for generic structs for now without schema awareness.
                 Err(std::io::Error::new(std::io::ErrorKind::Other, "Encoding generic structs not yet supported without schema"))
             }
         }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Uint(a), Value::Uint(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Struct(n1, f1), Value::Struct(n2, f2)) => n1 == n2 && f1 == f2,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        // basic implementation to allow use in BTreeMap keys
        // Order by type variant index, then content
        use Value::*;
        match (self, other) {
            (Nil, Nil) => Ordering::Equal,
            (Nil, _) => Ordering::Less,
            (_, Nil) => Ordering::Greater,
            
            (Bool(a), Bool(b)) => a.cmp(b),
            (Bool(_), _) => Ordering::Less,
            (_, Bool(_)) => Ordering::Greater,

            (Int(a), Int(b)) => a.cmp(b),
            (Int(_), _) => Ordering::Less,
            (_, Int(_)) => Ordering::Greater,
            
            (Uint(a), Uint(b)) => a.cmp(b),
            (Uint(_), _) => Ordering::Less,
            (_, Uint(_)) => Ordering::Greater,

            (Float(a), Float(b)) => a.to_bits().cmp(&b.to_bits()),
            (Float(_), _) => Ordering::Less,
            (_, Float(_)) => Ordering::Greater,

            (String(a), String(b)) => a.cmp(b),
            (String(_), _) => Ordering::Less,
            (_, String(_)) => Ordering::Greater,

            (Bytes(a), Bytes(b)) => a.cmp(b),
            (Bytes(_), _) => Ordering::Less,
            (_, Bytes(_)) => Ordering::Greater,

            (Array(a), Array(b)) => a.cmp(b),
            (Array(_), _) => Ordering::Less,
            (_, Array(_)) => Ordering::Greater,

            (Map(a), Map(b)) => a.cmp(b),
            (Map(_), _) => Ordering::Less,
            (_, Map(_)) => Ordering::Greater,
            
            (Struct(n1, f1), Struct(n2, f2)) => {
                match n1.cmp(n2) {
                    Ordering::Equal => f1.cmp(f2),
                    ord => ord,
                }
            }
        }
    }
}
