use byteorder::{BigEndian, ByteOrder};
use std::collections::{HashMap, BTreeMap};
use crate::Result;
use crate::value::Value;

#[derive(Debug, Clone)]
pub enum TypeSchema {
    Bool,
    Int,
    Uint,
    Float,
    ByteSlice,
    String,
    Interface,
    Map(i64, i64), // KeyID, ElemID
    Struct(Vec<(i64, i64, String)>), // (FieldDelta, TypeID, Name)
    Custom(i64), // Placeholder for user defined types
}

pub struct Decoder<R: std::io::Read> {
    reader: R,
    types: HashMap<i64, TypeSchema>,
    stash: Vec<u8>,
    current_msg_remaining: usize, 
}

impl<R: std::io::Read> Decoder<R> {
    pub fn new(reader: R) -> Self {
        let mut types = HashMap::new();
        types.insert(1, TypeSchema::Bool);
        types.insert(2, TypeSchema::Int);
        types.insert(3, TypeSchema::Uint);
        types.insert(4, TypeSchema::Float);
        types.insert(5, TypeSchema::ByteSlice);
        types.insert(6, TypeSchema::String);
        types.insert(8, TypeSchema::Interface);
        
        Self { 
            reader, 
            types, 
            stash: Vec::new(),
            current_msg_remaining: 0,
        }
    }

    fn read_raw_exact(&mut self, buf: &mut [u8]) -> Result<()> {
         self.reader.read_exact(buf)?;
         Ok(())
    }

    fn read_raw_u8(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.read_raw_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_raw_uint(&mut self) -> Result<u64> {
        let u7_or_len = self.read_raw_u8()?;
        if u7_or_len < 128 {
            return Ok(u7_or_len as u64);
        }
        let len = (!u7_or_len).wrapping_add(1) as usize;
        let mut buf = vec![0; len];
        self.read_raw_exact(&mut buf)?;
        Ok(BigEndian::read_uint(&buf, len))
    }
    
    fn process_next_message_header(&mut self) -> Result<()> {
        loop {
            // Read Msg Length
            let msg_len_res = self.read_raw_uint();
            if let Err(e) = msg_len_res {
                return Err(e); 
            }
            let msg_len = msg_len_res? as usize;
            
            self.current_msg_remaining = msg_len;
            
            let type_id = self.read_int()?;
            
            if type_id < 0 {
                let def_id = -type_id;
                let schema = self.decode_wire_type()?;
                self.types.insert(def_id, schema);
                
                if self.current_msg_remaining > 0 {
                    let mut drain = vec![0; self.current_msg_remaining];
                    self.read_raw_exact(&mut drain)?;
                    self.current_msg_remaining = 0;
                }
                continue;
            } else {
                return Ok(());
            }
        }
    }

    fn read_exact_internal(&mut self, buf: &mut [u8]) -> Result<()> {
        let mut pos = 0;
        
        while pos < buf.len() && !self.stash.is_empty() {
            buf[pos] = self.stash.remove(0);
            pos += 1;
        }
        
        while pos < buf.len() {
            if self.current_msg_remaining == 0 {
                if let Err(e) = self.process_next_message_header() {
                     return Err(e);
                }
            }
            
            let needed = buf.len() - pos;
            let to_read = std::cmp::min(needed, self.current_msg_remaining);
            
            if to_read > 0 {
                self.reader.read_exact(&mut buf[pos..pos+to_read])?;
                self.current_msg_remaining -= to_read;
                pos += to_read;
            }
        }
        Ok(())
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.read_exact_internal(&mut buf)?;
        Ok(buf[0])
    }

    #[inline]
    pub fn read_uint(&mut self) -> Result<u64> {
        let u7_or_len = self.read_u8()?;
        if u7_or_len < 128 {
            return Ok(u7_or_len as u64);
        }
        let len = (!u7_or_len).wrapping_add(1);
        self.fast_get_uint_be(len as usize)
    }
    
    fn fast_get_uint_be(&mut self, nbytes: usize) -> Result<u64> {
        let mut buf = vec![0; nbytes];
        self.read_exact_internal(&mut buf)?;
        Ok(BigEndian::read_uint(&buf[..nbytes], nbytes))
    }
    
    #[inline]
    pub fn read_int(&mut self) -> Result<i64> {
        let bits = self.read_uint()?;
        let sign = bits & 1;
        let sint = (bits >> 1) as i64;
        if sign == 0 {
            Ok(sint)
        } else {
            Ok(!sint)
        }
    }
    
    #[inline]
    pub fn read_float(&mut self) -> Result<f64> {
         let bits = self.read_uint()?;
         Ok(f64::from_bits(bits.swap_bytes()))
    }
    
    #[inline]
    pub fn read_bool(&mut self) -> Result<bool> {
        match self.read_uint()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "integer overflow")),
        }
    }
    
    pub fn read_bytes(&mut self) -> Result<Vec<u8>> {
        let len = self.read_uint()? as usize;
        let mut buf = vec![0; len];
        self.read_exact_internal(&mut buf)?;
        Ok(buf)
    }
    
    pub fn read_exact_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0; len];
        self.read_exact_internal(&mut buf)?;
        Ok(buf)
    }

    pub fn read_string(&mut self) -> Result<String> {
        let bytes = self.read_bytes()?;
        String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn read_next(&mut self) -> Result<Option<Value>> {
        if self.current_msg_remaining > 0 {
            let mut drain = vec![0; self.current_msg_remaining];
            self.read_raw_exact(&mut drain)?;
            self.current_msg_remaining = 0;
        }

        loop {
            let msg_len_res = self.read_raw_uint();
            if let Err(e) = msg_len_res {
                 if e.kind() == std::io::ErrorKind::UnexpectedEof {
                     return Ok(None);
                 }
                 return Err(e);
            }
            let msg_len = msg_len_res? as usize;
            self.current_msg_remaining = msg_len;
            
            let type_id = self.read_int()?;
            
            if type_id < 0 {
                let def_id = -type_id;
                let schema = self.decode_wire_type()?;
                self.types.insert(def_id, schema);
                
                if self.current_msg_remaining > 0 {
                     let mut drain = vec![0; self.current_msg_remaining];
                     self.read_raw_exact(&mut drain)?;
                     self.current_msg_remaining = 0;
                }
                continue;
            } else {
                 if let Some(schema) = self.types.get(&type_id).cloned() {
                     if type_id == 64 {
                         let b = self.read_u8()?;
                         if b != 0 {
                             self.stash.push(b);
                         }
                    }
                    
                    let val = self.decode_value(&schema)?;
                    
                    if self.current_msg_remaining > 0 {
                         let mut drain = vec![0; self.current_msg_remaining];
                         self.read_raw_exact(&mut drain)?;
                         self.current_msg_remaining = 0;
                    }
                    
                    return Ok(Some(val));
                } else {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown type ID: {}", type_id)));
                }
            }
        }
    }
    
    fn decode_wire_type(&mut self) -> Result<TypeSchema> {
         let mut schema = TypeSchema::Interface; 
         let mut field_num = -1;
         loop {
             let delta = self.read_uint()?;
             if delta == 0 { return Ok(schema); }
             field_num += delta as i64;
             
             match field_num {
                 0 => { return Err(std::io::Error::new(std::io::ErrorKind::Other, "ArrayT not impl")); }
                 1 => { return Err(std::io::Error::new(std::io::ErrorKind::Other, "SliceT not impl")); }
                 2 => { schema = self.decode_struct_type()?; }
                 3 => { schema = self.decode_map_type()?; }
                 4 => { return Err(std::io::Error::new(std::io::ErrorKind::Other, "GobEncoderT not impl")); }
                 _ => { return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Unknown WireType field {}", field_num))); }
             }
         }
    }

    fn decode_map_type(&mut self) -> Result<TypeSchema> {
        let mut key_id = 0;
        let mut elem_id = 0;
        let mut field_num = -1;
        loop {
            let delta = self.read_uint()?;
            if delta == 0 { break; }
            field_num += delta as i64;
            match field_num {
                0 => {
                    let mut ct_field = -1;
                    loop {
                        let ct_delta = self.read_uint()?;
                        if ct_delta == 0 { break; }
                        ct_field += ct_delta as i64;
                        match ct_field {
                            0 => { let _ = self.read_string()?; }
                            1 => { let _ = self.read_int()?; }
                            _ => {}
                        }
                    }
                }
                1 => { key_id = self.read_int()?; }
                2 => { elem_id = self.read_int()?; }
                _ => {}
            }
        }
        Ok(TypeSchema::Map(key_id, elem_id))
    }

    fn decode_struct_type(&mut self) -> Result<TypeSchema> {
         let mut fields = Vec::new();
         let mut field_num = -1;
         loop {
             let delta = self.read_uint()?;
             if delta == 0 { break; }
             field_num += delta as i64;
             match field_num {
                 0 => {
                     let mut ct_field = -1;
                     loop {
                         let ct_delta = self.read_uint()?;
                         if ct_delta == 0 { break; }
                         ct_field += ct_delta as i64;
                         match ct_field {
                             0 => { let _ = self.read_string()?; } 
                             1 => { let _ = self.read_int()?; }
                             _ => {}
                         }
                     }
                 }
                 1 => {
                     let count = self.read_uint()?;
                     for _ in 0..count {
                         let mut ft_field = -1;
                         let mut name = String::new();
                         let mut id = 0;
                         loop {
                             let ft_delta = self.read_uint()?;
                             if ft_delta == 0 { break; }
                             ft_field += ft_delta as i64;
                             match ft_field {
                                 0 => { name = self.read_string()?; } 
                                 1 => { id = self.read_int()?; }
                                 _ => {}
                             }
                         }
                         fields.push((0, id, name));
                     }
                 }
                 _ => {}
             }
         }
         Ok(TypeSchema::Struct(fields))
    }
    
    fn decode_value(&mut self, schema: &TypeSchema) -> Result<Value> {
        match schema {
            TypeSchema::Bool => Ok(Value::Bool(self.read_bool()?)),
            TypeSchema::Int => Ok(Value::Int(self.read_int()?)),
            TypeSchema::Uint => Ok(Value::Uint(self.read_uint()?)),
            TypeSchema::Float => Ok(Value::Float(self.read_float()?)),
            TypeSchema::String => Ok(Value::String(self.read_string()?)),
            TypeSchema::ByteSlice => Ok(Value::Bytes(self.read_bytes()?)),
            TypeSchema::Map(kid, vid) => {
                let count = self.read_uint()?;
                self.decode_map_body(count, *kid, *vid)
            }
            TypeSchema::Struct(fields) => {
                let mut struct_val = BTreeMap::new();
                let mut field_idx = -1;
                loop {
                    let delta = self.read_uint()?;
                    if delta == 0 { break; }
                    field_idx += delta as i64;
                    if field_idx >= 0 && (field_idx as usize) < fields.len() {
                        let (_, type_id, name) = &fields[field_idx as usize];
                        if let Some(field_schema) = self.types.get(type_id).cloned() {
                             let val = self.decode_value(&field_schema)?;
                             struct_val.insert(name.clone(), val);
                        } else {
                             return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown type for struct field {}", name)));
                        }
                    } else {
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown field index {} for Struct", field_idx)));
                    }
                }
                Ok(Value::Struct("Struct".to_string(), struct_val)) 
            }
            TypeSchema::Interface => {
                self.decode_interface()
            }
            _ => {
                Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Unimplemented decoder for {:?}", schema)))
            }
        }
    }

    fn decode_map_body(&mut self, count: u64, kid: i64, vid: i64) -> Result<Value> {
        let k_schema = self.types.get(&kid).cloned().unwrap_or(TypeSchema::Custom(kid));
        let v_schema = self.types.get(&vid).cloned().unwrap_or(TypeSchema::Custom(vid));
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let k = self.decode_value(&k_schema)?;
            let v = self.decode_value(&v_schema)?;
            map.insert(k, v);
        }
        Ok(Value::Map(map))
    }

    pub fn decode_interface(&mut self) -> Result<Value> {
        let name = self.read_string()?;
        if name.is_empty() { return Ok(Value::Nil); }
        
        let mut type_id = self.read_int()?;
        if type_id < 0 {
            let def_id = -type_id;
            let schema = self.decode_wire_type()?;
            self.types.insert(def_id, schema);
            type_id = def_id;
        }

        let len = self.read_uint()? as usize;
        
        let b = self.read_u8()?;
        if b != 0 {
            self.stash.push(b);
        }

        let result;
        match name.as_str() {
            "string" => { result = Ok(Value::String(self.read_string()?)); }
            "int" | "int64" | "uint" => { result = Ok(Value::Int(self.read_int()?)); }
            "bool" => { result = Ok(Value::Bool(self.read_bool()?)); }
            "float64" => { result = Ok(Value::Float(self.read_float()?)); }
            _ => {
                if let Some(schema) = self.types.get(&type_id).cloned() {
                    if len > 0 {
                        let mut val = self.decode_value(&schema)?;
                        if let Value::Struct(_, fields) = val {
                            val = Value::Struct(name.clone(), fields);
                        }
                        result = Ok(val);
                    } else {
                        result = Ok(Value::Nil);
                    }
                } else {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown concrete type definition for interface: {} (ID {})", name, type_id)));
                }
            }
        }
        
        result
    }
    
    pub fn parse(&mut self) -> Result<()> {
        while let Some(v) = self.read_next()? {
            println!("Decoded Value: {:?}", v);
        }
        Ok(())
    }
    
    pub fn decode_into<T: GobDecodable>(&mut self) -> Result<T> {
        // We need to advance to the next value message.
        // This involves reading headers and processing type definitions.
        
        loop {
            // Read Msg Length
            let msg_len_res = self.read_raw_uint();
            if let Err(e) = msg_len_res {
                 return Err(e); 
            }
            let msg_len = msg_len_res? as usize;
            
            self.current_msg_remaining = msg_len;
            
            let type_id = self.read_int()?;
            println!("DEBUG: Msg Len: {}, Type ID: {}", msg_len, type_id);
            
            if type_id < 0 {
                // Type definition
                let def_id = -type_id;
                let schema = self.decode_wire_type()?;
                self.types.insert(def_id, schema);
                
                if self.current_msg_remaining > 0 {
                    let mut drain = vec![0; self.current_msg_remaining];
                    self.read_raw_exact(&mut drain)?;
                    self.current_msg_remaining = 0;
                }
                continue;
            } else {
                // Value message!
                // We are now positioned at the start of the value content.
                
                // Hack from read_next: Special handling for type 64?
                if type_id == 64 {
                     let b = self.read_u8()?;
                     if b != 0 {
                         self.stash.push(b);
                     }
                }

                // We delegate to T::decode.
                // Note: We ignore type_id for now, assuming T knows how to decode itself
                // matching the wire format. In a robust implementation, we would check type_id compatibility.
                
                // Also, we need to handle the `ignore` byte if type_id == 64? No, that's handled inside decode_interface usually?
                // Wait, type_id 64 is likely not used for custom structs directly unless they are wire types?
                // For standard values, we just decode.
                
                let val = T::decode(self)?;
                
                // Ensure we drain any remaining bytes of the message
                if self.current_msg_remaining > 0 {
                     let mut drain = vec![0; self.current_msg_remaining];
                     self.read_raw_exact(&mut drain)?;
                     self.current_msg_remaining = 0;
                }
                
                return Ok(val);
            }
        }
    }
}

pub trait GobDecodable: Sized {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self>;
}

impl GobDecodable for bool {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self> {
        decoder.read_bool()
    }
}

impl GobDecodable for i64 {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self> {
        decoder.read_int()
    }
}

impl GobDecodable for u64 {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self> {
        decoder.read_uint()
    }
}

impl GobDecodable for f64 {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self> {
        decoder.read_float()
    }
}

impl GobDecodable for String {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self> {
        decoder.read_string()
    }
}

impl GobDecodable for Vec<u8> {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self> {
        decoder.read_bytes()
    }
}

impl GobDecodable for Value {
    fn decode<R: std::io::Read>(decoder: &mut Decoder<R>) -> Result<Self> {
        // We use read_next which handles message headers and type definitions.
        // But read_next returns Option<Value>.
        // If we get None, it's EOF.
        // In the context of "decode a value", we probably expect one to be there.
        // However, standard Gob stream is a sequence of messages.
        // If we are "decoding a map element", we are already inside a message?
        // No, map elements are values inside a message.
        // Decoder::read_next() is for top-level messages.
        // BUT, `decode_value` recursively calls `decode_value`.
        // We need `decode_next_value` which might be internal or exposed?
        
        // Wait, the macro uses `gobx::Value::decode(decoder)`.
        // If we are inside a map, we are decoding map elements.
        // Map elements are NOT top-level messages with type definitions (unless interface{}?).
        // If the map type is map[string]int, the elements are string and int.
        // If the map type is map[interface{}]interface{}, the elements are Interface values.
        
        // Interface values ARE self-describing (name + type definition + value).
        // Our `decode_interface` handles this.
        
        // So if we are in `interpret_as="map[interface{}]interface{}"`, the keys and values are interfaces.
        // So we should call something that reads an interface.
        // OR, simply `decoder.read_next()`?
        // `read_next` expects the length + type_id header of a top-level message.
        // Interface values on the wire ALSO look like that?
        // Let's check `decode_interface`:
        // reads name, then type_id, then length (sometimes).
        
        // If we use `read_next` inside a struct decode, it will try to read a length prefix.
        // BUT inside a struct/map, values usually don't have length prefix unless they are messages?
        // Actually, in Gob, only top-level values are "messages".
        // Inner values are just encoded.
        // EXCEPT interfaces, which carry type info.
        
        // If the macro generates code for `interpret_as` map, it reads `count`.
        // Then it loops.
        // Inside loop, it reads Key and Value.
        // If the map is map[interface]interface, then Key and Value are encoded as Interface.
        // Interface encoding:
        // [Name len] [Name bytes] [TypeID] [Value] (roughly)
        
        // `Decoder::decode_value` handles schema-based decoding.
        // But here we are decoding into a `Value` enum without knowing the schema beforehand?
        // We need to know what we are reading.
        // If we are `map[interface{}]interface{}`, the schema says "Interface".
        // So we should call `decoder.decode_interface()`.
        
        // But `GobDecodable::decode` is generic.
        // If we implement `GobDecodable` for `Value`, what should it do?
        // It can't know if it should read an int, string, or interface, unless it knows the expected type.
        // But `Value` is "Any".
        // The only "Any" type in Gob is Interface.
        // So `Value::decode` should probably behave like reading an Interface?
        
        // Let's check usage in macro:
        // `let key_val = gobx::Value::decode(decoder)?;`
        // It assumes the next thing on wire is an interface (because we are in map[interface]interface).
        
        // So yes, `Value::decode` should call `decoder.decode_interface()`.
        // BUT `decode_interface` is private. We need to expose it or wrap it.
        // OR `Decoder` needs a public `read_value` that reads a value given a schema?
        // But we don't have schema passed to `GobDecodable::decode`.
        
        // Conclusion: `GobDecodable` is for types where the structure is known (static types).
        // `Value` corresponds to `interface{}` (dynamic type).
        // So `Value::decode` should decode an Interface wire format.
        
        decoder.decode_interface()
    }
}
