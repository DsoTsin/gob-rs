use serde::{ser, Serialize};
use crate::{Encoder, Result};
use std::io::Write;

pub struct Serializer<'a, W: Write> {
    encoder: &'a mut Encoder<W>,
}

impl<'a, W: Write> Serializer<'a, W> {
    pub fn new(encoder: &'a mut Encoder<W>) -> Self {
        Serializer { encoder }
    }
}

impl<'a, W: Write> ser::Serializer for Serializer<'a, W> {
    type Ok = ();
    type Error = std::io::Error; // Use io::Error or wrapper

    type SerializeSeq = ser::Impossible<(), Self::Error>; // TODO
    type SerializeTuple = ser::Impossible<(), Self::Error>;
    type SerializeTupleStruct = ser::Impossible<(), Self::Error>;
    type SerializeTupleVariant = ser::Impossible<(), Self::Error>;
    type SerializeMap = ser::Impossible<(), Self::Error>;
    type SerializeStruct = ser::Impossible<(), Self::Error>;
    type SerializeStructVariant = ser::Impossible<(), Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        self.encoder.write_bool(v)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok> {
        self.encoder.write_int(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok> {
        self.encoder.write_int(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok> {
        self.encoder.write_int(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok> {
        self.encoder.write_int(v)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok> {
        self.encoder.write_uint(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok> {
        self.encoder.write_uint(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        self.encoder.write_uint(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        self.encoder.write_uint(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok> {
        self.encoder.write_float(v as f64)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok> {
        self.encoder.write_float(v)
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        self.encoder.write_int(v as i64) // Gob treats chars often as ints or strings? Go rune is int32.
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        self.encoder.write_string(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok> {
        self.encoder.write_bytes(v)
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        Ok(()) // Nil in gob? Often context dependent.
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok> {
        // Enums not directly mapping to gob without more info
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Enum variants not supported yet"))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok>
    where
        T: Serialize,
    {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Enum variants not supported yet"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Seq not supported yet"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Tuple not supported yet"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "TupleStruct not supported yet"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "TupleVariant not supported yet"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Map not supported yet"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Struct not supported yet"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "StructVariant not supported yet"))
    }
}

