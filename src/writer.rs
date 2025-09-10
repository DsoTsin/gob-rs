use std::collections::{HashMap, BTreeMap};
use std::io::{Write, Seek, Cursor};
use crate::{Encoder, Result, Value};
use crate::decode::TypeSchema;

pub struct GobWriter<W: Write> {
    encoder: Encoder<W>,
    type_ids: HashMap<String, i64>, // Name/Signature -> ID
    next_id: i64,
}

impl<W: Write> GobWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            encoder: Encoder::new(writer),
            type_ids: HashMap::new(),
            next_id: 65,
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        self.encoder.flush()
    }

    fn get_type_id(&mut self, schema_key: &str) -> Option<i64> {
        self.type_ids.get(schema_key).cloned()
    }

    fn assign_type_id(&mut self, schema_key: String) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.type_ids.insert(schema_key, id);
        id
    }

    // High level encode
    pub fn encode(&mut self, value: &Value) -> Result<()> {
        // We treat the top level value as the message.
        // We usually assume it's a Map or Struct.
        
        // 1. Determine Type ID and ensure definition is sent.
        let type_id = self.ensure_type_defined(value)?;

        // 2. Encode Message: [Length] [TypeID] [Value]
        // We need to capture the value bytes to know length.
        let mut value_buf = Vec::new();
        let mut sub_writer = GobWriter::new(&mut value_buf);
        // Share type registry? 
        // Ideally yes, but for simplicity, let's assume we pass down context or re-use writer logic without creating new structs.
        // Actually, we need to separate "Encode Definition" from "Encode Value".
        
        // Let's refactor: `encode_value_content` writes into a buffer.
        let mut content_buf = Vec::new();
        {
             let mut sub_encoder = Encoder::new(&mut content_buf);
             self.encode_value_body(&mut sub_encoder, value, type_id)?;
        }

        // 3. Write Length
        // Length covers TypeID + Content.
        // We need to encode TypeID into bytes to measure it?
        // Wait, TypeID is just an Int.
        // [Length of (TypeID + Content)] [TypeID] [Content]
        
        let mut type_id_buf = Vec::new();
        let mut type_id_enc = Encoder::new(&mut type_id_buf);
        type_id_enc.write_int(type_id)?;
        
        let total_len = type_id_buf.len() + content_buf.len();
        self.encoder.write_uint(total_len as u64)?;
        self.encoder.write_all(&type_id_buf)?;
        self.encoder.write_all(&content_buf)?;
        
        Ok(())
    }

    fn ensure_type_defined(&mut self, value: &Value) -> Result<i64> {
        match value {
            Value::Bool(_) => Ok(1),
            Value::Int(_) => Ok(2),
            Value::Uint(_) => Ok(3),
            Value::Float(_) => Ok(4),
            Value::Bytes(_) => Ok(5),
            Value::String(_) => Ok(6),
            Value::Map(_) => {
                // Assume Map<interface{}, interface{}> for generic map
                let key = "Map(8,8)".to_string();
                if let Some(id) = self.get_type_id(&key) {
                    return Ok(id);
                }
                
                let id = self.assign_type_id(key);
                self.send_map_type_def(id, 8, 8)?;
                Ok(id)
            }
            Value::Struct(name, fields) => {
                // We need a signature for the struct logic.
                // Using name is risky if different structs have same name.
                // But gob assumes name uniqueness often or structure uniqueness.
                // Let's use name for now.
                // Note: Fields need to be sorted for deterministic signature?
                // BTreeMap sorts by key.
                
                if let Some(id) = self.get_type_id(name) {
                    return Ok(id);
                }

                // We must define field types first.
                // This might be recursive.
                let mut field_defs = Vec::new();
                for (fname, fval) in fields {
                    let fid = self.ensure_type_defined(fval)?;
                    field_defs.push((fname.clone(), fid));
                }

                let id = self.assign_type_id(name.clone());
                self.send_struct_type_def(id, name, field_defs)?;
                Ok(id)
            }
            Value::Array(_) => Err(std::io::Error::new(std::io::ErrorKind::Other, "Array encode not impl")),
            Value::Nil => Ok(0), // ?
        }
    }

    fn send_map_type_def(&mut self, id: i64, key_id: i64, elem_id: i64) -> Result<()> {
        // Definition is a message with ID = -id
        // Content is WireType.
        // WireType { MapT: MapType { Key: key_id, Elem: elem_id } }
        
        let mut content = Vec::new();
        let mut enc = Encoder::new(&mut content);
        
        // WireType is a struct.
        // Field 3 is MapT.
        // Delta = 3 + 1 (field num is -1 based in some contexts? No, Decoder says field_num = -1 + delta)
        // MapT is field 3.
        // Delta = 3 - (-1) = 4.
        enc.write_uint(4)?; 
        
        // MapType struct:
        // Field 0: CommonType (name, id). We usually skip or write empty?
        // Decoder: Field 0 (CommonType) -> ignored/read.
        // Field 1: KeyID
        // Field 2: ElemID
        
        // We write KeyID (Field 1).
        // Delta = 1 - (-1) = 2.
        enc.write_uint(2)?;
        enc.write_int(key_id)?;
        
        // ElemID (Field 2).
        // Delta = 2 - 1 = 1.
        enc.write_uint(1)?;
        enc.write_int(elem_id)?;
        
        // End of MapType struct
        enc.write_uint(0)?;
        
        // End of WireType struct
        enc.write_uint(0)?;
        
        // Write Message
        let mut type_id_buf = Vec::new();
        let mut t_enc = Encoder::new(&mut type_id_buf);
        t_enc.write_int(-id)?; // Negative for definition
        
        let len = type_id_buf.len() + content.len();
        self.encoder.write_uint(len as u64)?;
        self.encoder.write_all(&type_id_buf)?;
        self.encoder.write_all(&content)?;
        
        Ok(())
    }

    fn send_struct_type_def(&mut self, id: i64, name: &str, fields: Vec<(String, i64)>) -> Result<()> {
        // WireType { StructT: StructType { CommonType: { Name: name, Id: id }, Fields: [...] } }
        
        let mut content = Vec::new();
        let mut enc = Encoder::new(&mut content);
        
        // WireType Field 2 is StructT.
        // Delta = 2 - (-1) = 3.
        enc.write_uint(3)?;
        
        // StructType struct:
        // Field 0: CommonType
        // Field 1: Fields (Slice)
        
        // Write CommonType (Field 0)
        // Delta = 0 - (-1) = 1.
        enc.write_uint(1)?;
        
        // CommonType struct:
        // Field 0: Name
        // Field 1: Id
        
        // Name (Field 0)
        // Delta = 1.
        enc.write_uint(1)?;
        enc.write_string(name)?;
        
        // Id (Field 1)
        // Delta = 1 - 0 = 1.
        enc.write_uint(1)?;
        enc.write_int(id)?;
        
        // End CommonType
        enc.write_uint(0)?;
        
        // Write Fields (Field 1 of StructType)
        // Delta = 1 - 0 = 1.
        enc.write_uint(1)?;
        
        // Slice length
        enc.write_uint(fields.len() as u64)?;
        
        for (fname, fid) in fields {
            // FieldType struct:
            // Field 0: Name
            // Field 1: Id
            
            // Name (Field 0)
            enc.write_uint(1)?;
            enc.write_string(&fname)?;
            
            // Id (Field 1)
            enc.write_uint(1)?;
            enc.write_int(fid)?;
            
            // End FieldType
            enc.write_uint(0)?;
        }
        
        // End StructType
        enc.write_uint(0)?;
        
        // End WireType
        enc.write_uint(0)?;
        
        // Send Message
        let mut type_id_buf = Vec::new();
        let mut t_enc = Encoder::new(&mut type_id_buf);
        t_enc.write_int(-id)?;
        
        let len = type_id_buf.len() + content.len();
        self.encoder.write_uint(len as u64)?;
        self.encoder.write_all(&type_id_buf)?;
        self.encoder.write_all(&content)?;
        
        Ok(())
    }

    fn encode_value_body<E: Write>(&mut self, enc: &mut Encoder<E>, value: &Value, type_id: i64) -> Result<()> {
        // This encodes the "payload" of the value.
        // Structure depends on schema.
        
        match value {
            Value::Bool(v) => enc.write_bool(*v)?,
            Value::Int(v) => enc.write_int(*v)?,
            Value::Uint(v) => enc.write_uint(*v)?,
            Value::Float(v) => enc.write_float(*v)?,
            Value::String(v) => enc.write_string(v)?,
            Value::Bytes(v) => enc.write_bytes(v)?,
            Value::Map(m) => {
                // Map encoding: Count, then (Key, Val) pairs.
                enc.write_uint(m.len() as u64)?;
                for (k, v) in m {
                    // For Map<interface, interface>, we need to encode values AS interfaces.
                    // This means wrapping them.
                    self.encode_interface_value(enc, k)?;
                    self.encode_interface_value(enc, v)?;
                }
            },
            Value::Struct(_, fields) => {
                // Struct encoding: Field deltas.
                // We assume `fields` contains all fields defined in the type, in order?
                // Or we need to map names to indices.
                // But `Value::Struct` is BTreeMap (sorted by name).
                // Our `send_struct_type_def` used iteration order of BTreeMap (sorted).
                // So field indices are 0, 1, 2... in name-sorted order.
                
                let mut current_idx = -1;
                let mut idx = 0;
                for (name, val) in fields {
                     // Check if not nil/empty/zero? Gob omits zero values.
                     // For now, send everything.
                     
                     let delta = (idx as i64) - current_idx;
                     enc.write_uint(delta as u64)?;
                     current_idx = idx as i64;
                     
                     // Encode field value
                     // If field is interface? We need schema to know.
                     // But we are constructing schema on fly.
                     // If `val` matches the `fid` we used in definition.
                     // `fid` came from `ensure_type_defined`.
                     // If `val` is struct/map, `fid` is concrete type ID.
                     // If the FIELD TYPE was defined as interface, we wrap.
                     // BUT here we defined the field type AS the concrete type ID!
                     // So we don't wrap?
                     
                     // Wait. In `ensure_type_defined` for Struct:
                     // `let fid = self.ensure_type_defined(fval)?;`
                     // This returns the CONCRETE type ID of the value.
                     // So we defined the struct as having fields of these specific concrete types.
                     // So we do NOT wrap in interface.
                     // We just encode the body recursively.
                     let fid = self.ensure_type_defined(val)?;
                     self.encode_value_body(enc, val, fid)?;
                     
                     idx += 1;
                }
                enc.write_uint(0)?; // End of struct
            },
             _ => {}
        }
        Ok(())
    }

    fn encode_interface_value<E: Write>(&mut self, enc: &mut Encoder<E>, value: &Value) -> Result<()> {
        // Interface encoding: Name, TypeID, Length, Value.
        
        // 1. Concrete Name
        let name = match value {
            Value::Bool(_) => "bool",
            Value::Int(_) => "int64", // Standard for gob numbers is often int64? Go decoder saw "int64" for 1, and "int" for -1?
            Value::Uint(_) => "uint",
            Value::Float(_) => "float64",
            Value::String(_) => "string",
            Value::Bytes(_) => "[]byte",
            Value::Struct(n, _) => n,
            Value::Map(_) => "map[interface{}]interface{}", // Approximate
            Value::Nil => "",
            _ => "unknown",
        };
        
        enc.write_string(name)?;
        if name == "" { return Ok(()); }
        
        // 2. Concrete Type ID.
        // We might need to send definition if not sent.
        // Since we are inside a message body, can we send definitions interleaved?
        // No, definitions must be top level messages?
        // Actually, gob allows definitions inside the stream, interleaved with values?
        // Yes, my Decoder handles "Refill".
        // BUT, we are currently writing into `content_buf` which is inside a message.
        // Can we insert a definition INSIDE a message?
        // No, definitions are distinct messages.
        // So we must have ensured definitions were sent BEFORE we started this message.
        // `ensure_type_defined` should have been called recursively?
        // Yes, `ensure_type_defined(value)` recursively defines sub-types.
        // BUT, `encode` calls `ensure_type_defined` on top value.
        // Does it recurse?
        // `ensure_type_defined` for Map/Struct DOES recurse.
        // So all types should be defined.
        
        let type_id = self.ensure_type_defined(value)?;
        enc.write_int(type_id)?;
        
        // 3. Length of value
        let mut val_buf = Vec::new();
        let mut val_enc = Encoder::new(&mut val_buf);
        
        // 00 byte skip rule for interfaces?
        // My decoder checks for 0 byte.
        // Go gob decoder expects 0 byte if the value is NOT empty?
        // Actually, gob spec: "Interface values are encoded as... Length... Value".
        // The value itself might start with 0?
        // But my decoder logic: `let b = self.read_u8()?; if b != 0 { stash }`.
        // This implies sometimes there IS a 0 byte that is NOT part of the value?
        // No, it implies that the first byte MIGHT be 0, and if so we assume it's part of the stream (or skip?).
        // Actually, the `read_u8` then `stash` implies we just peeked.
        // It does NOT imply we skipped.
        // So we write standard value.
        
        self.encode_value_body(&mut val_enc, value, type_id)?;
        
        enc.write_uint(val_buf.len() as u64)?;
        enc.write_all(&val_buf)?;
        
        Ok(())
    }
}

