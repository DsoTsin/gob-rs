use std::io::Seek;
use byteorder::{BigEndian, ByteOrder};
use std::collections::HashMap;
use crate::Result;

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

pub struct Decoder<R: std::io::Read + std::io::Seek> {
    reader: R,
    types: HashMap<i64, TypeSchema>,
    stash: Vec<u8>,
}

impl<R: std::io::Read + std::io::Seek> Decoder<R> {
    pub fn new(reader: R) -> Self {
        let mut types = HashMap::new();
        types.insert(1, TypeSchema::Bool);
        types.insert(2, TypeSchema::Int);
        types.insert(3, TypeSchema::Uint);
        types.insert(4, TypeSchema::Float);
        types.insert(5, TypeSchema::ByteSlice);
        types.insert(6, TypeSchema::String);
        types.insert(8, TypeSchema::Interface);
        
        Self { reader, types, stash: Vec::new() }
    }

    fn read_exact_internal(&mut self, buf: &mut [u8]) -> Result<()> {
        let mut pos = 0;
        while pos < buf.len() && !self.stash.is_empty() {
            buf[pos] = self.stash.remove(0);
            pos += 1;
        }
        if pos < buf.len() {
            self.reader.read_exact(&mut buf[pos..])?;
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
    
    pub fn parse(&mut self) -> Result<()> {
        loop {
             let msg_length_res = self.read_uint();
             if let Err(e) = msg_length_res {
                 if e.kind() == std::io::ErrorKind::UnexpectedEof {
                     break;
                 }
                 break;
             }
             let msg_length = msg_length_res? as usize;

            let mut payload = vec![0; msg_length];
            self.reader.read_exact(&mut payload)?;

            let mut cursor = std::io::Cursor::new(payload);
            let mut msg_decoder = Decoder::new(&mut cursor);
            msg_decoder.types = self.types.clone(); 

            let type_id = msg_decoder.read_int()?;
            
            if type_id < 0 {
                let def_id = -type_id;
                println!("Definition of Type ID {}", def_id);
                match msg_decoder.decode_wire_type() {
                    Ok(schema) => {
                        println!("Parsed Schema for {}: {:?}", def_id, schema);
                        self.types.insert(def_id, schema);
                    }
                    Err(e) => {
                        println!("Failed to parse WireType for {}: {:?}", def_id, e);
                        if def_id == 64 {
                             self.types.insert(64, TypeSchema::Map(8, 8));
                        }
                    }
                }
            } else {
                println!("Value of Type ID {}", type_id);
                if let Some(schema) = self.types.get(&type_id).cloned() {
                    if type_id == 64 {
                         let pos = msg_decoder.reader.stream_position()?;
                         if let Ok(b) = msg_decoder.read_u8() {
                            if b != 0 {
                                msg_decoder.reader.seek(std::io::SeekFrom::Start(pos))?;
                            }
                         }
                    }
                    msg_decoder.types = self.types.clone();
                    
                    if let Err(e) = msg_decoder.decode_value(&schema) {
                        println!("Error decoding value: {:?}", e);
                    }
                    // Merge back types from message decoder
                    for (k, v) in msg_decoder.types.iter() {
                        self.types.entry(*k).or_insert_with(|| v.clone());
                    }
                } else {
                    println!("Unknown type ID: {}", type_id);
                }
            }
        }
        Ok(())
    }

    fn decode_wire_type(&mut self) -> Result<TypeSchema> {
         let delta = self.read_uint()?;
         let field_num = -1 + (delta as i64);
         
         match field_num {
             0 => { Err(std::io::Error::new(std::io::ErrorKind::Other, "ArrayT not impl")) }
             1 => { Err(std::io::Error::new(std::io::ErrorKind::Other, "SliceT not impl")) }
             2 => {
                 let _common_delta = self.read_uint()?; 
                 self.decode_struct_type()
             }
             3 => {
                 self.decode_map_type()
             }
             4 => { Err(std::io::Error::new(std::io::ErrorKind::Other, "GobEncoderT not impl")) }
             _ => { Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Unknown WireType field {}", field_num))) }
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
                _ => { println!("Unknown MapT field {}", field_num); }
            }
        }
        println!("Parsed MapT: Key {}, Elem {}", key_id, elem_id);
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
    
    fn decode_value(&mut self, schema: &TypeSchema) -> Result<()> {
        match schema {
            TypeSchema::Bool => {
                let v = self.read_bool()?;
                println!("Bool: {}", v);
            }
            TypeSchema::Int => {
                let v = self.read_int()?;
                println!("Int: {}", v);
            }
            TypeSchema::Uint => {
                let v = self.read_uint()?;
                println!("Uint: {}", v);
            }
            TypeSchema::Float => {
                let v = self.read_float()?;
                println!("Float: {}", v);
            }
            TypeSchema::String => {
                let v = self.read_string()?;
                println!("String: {:?}", v);
            }
            TypeSchema::Map(kid, vid) => {
                let count = self.read_uint()?;
                println!("Map count: {}", count);
                self.decode_map_body(count, *kid, *vid)?;
            }
            TypeSchema::Struct(fields) => {
                let mut field_idx = -1;
                loop {
                    let delta = self.read_uint()?;
                    if delta == 0 { break; }
                    field_idx += delta as i64;
                    
                    if field_idx >= 0 && (field_idx as usize) < fields.len() {
                        let (_, type_id, name) = &fields[field_idx as usize];
                        println!("Struct Field {}: {}", name, field_idx);
                        
                        if let Some(field_schema) = self.types.get(type_id).cloned() {
                             self.decode_value(&field_schema)?;
                        } else {
                             println!("Unknown Type ID {} for field {}", type_id, name);
                             return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown type for struct field"));
                        }
                    } else {
                        println!("Unknown field index {} for Struct", field_idx);
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown struct field"));
                    }
                }
            }
            TypeSchema::Interface => {
                self.decode_interface()?;
            }
            _ => {
                println!("Unimplemented decoder for {:?}", schema);
            }
        }
        Ok(())
    }

    fn decode_map_body(&mut self, count: u64, kid: i64, vid: i64) -> Result<()> {
        let k_schema = self.types.get(&kid).cloned().unwrap_or(TypeSchema::Custom(kid));
        let v_schema = self.types.get(&vid).cloned().unwrap_or(TypeSchema::Custom(vid));
        
        for _ in 0..count {
            self.decode_value(&k_schema)?;
            self.decode_value(&v_schema)?;
        }
        Ok(())
    }

    fn decode_interface(&mut self) -> Result<()> {
        let name = self.read_string()?;
        if name.is_empty() {
            println!("Interface: nil");
            return Ok(());
        }
        
        // Read Type ID (int). If negative, it's a definition.
        let mut type_id = self.read_int()?;
        if type_id < 0 {
            let def_id = -type_id;
            println!("Inline definition for ID {}", def_id);
            let schema = self.decode_wire_type()?;
            println!("Parsed Inline Schema for {}: {:?}", def_id, schema);
            self.types.insert(def_id, schema);
            type_id = def_id;
        }

        // Read Length (uint)
        let len = self.read_uint()? as usize;
        
        // Make sure we have enough bytes
        let mut payload = vec![0; len];
        self.read_exact_internal(&mut payload)?;
        
        let mut cursor = std::io::Cursor::new(payload.clone());
        let mut val_decoder = Decoder::new(&mut cursor);
        val_decoder.types = self.types.clone();
        
        // Generic 00 skip rule
        let pos = val_decoder.reader.position();
        if let Ok(b) = val_decoder.read_u8() {
            if b != 0 {
                val_decoder.reader.set_position(pos);
            }
        }

        println!("Interface Concrete Type: {}", name);
        match name.as_str() {
            "string" => {
                let v = val_decoder.read_string()?;
                println!("Key/Value: {} (String)", v);
            }
            "int" | "int64" | "uint" => {
                  let v = val_decoder.read_int()?;
                  println!("Key/Value: {} (Int)", v);
            }
            "bool" => {
                  let v = val_decoder.read_bool()?;
                  println!("Key/Value: {} (Bool)", v);
            }
            "float64" => {
                 let v = val_decoder.read_float()?;
                 println!("Key/Value: {} (Float)", v);
            }
            _ => {
                if let Some(schema) = self.types.get(&type_id).cloned() {
                    println!("Decoding custom type {} (ID {})", name, type_id);
                    if len > 0 {
                        val_decoder.decode_value(&schema)?;
                        // Merge back types?
                        for (k, v) in val_decoder.types.iter() {
                            self.types.insert(*k, v.clone());
                        }
                    } else {
                        println!("Key/Value: nil ({})", name);
                    }
                } else {
                    println!("Unknown concrete type definition for interface: {} (ID {})", name, type_id);
                }
            }
        }

        // PUSH BACK REMAINDER
        let pos = val_decoder.reader.position() as usize;
        if pos < payload.len() {
             println!("Pushing back {} bytes", payload.len() - pos);
             let rem = &payload[pos..];
             self.stash.extend_from_slice(rem);
        }

        Ok(())
    }
}
