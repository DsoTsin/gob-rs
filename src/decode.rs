use std::io::{self, Seek};
use byteorder::{BigEndian, ReadBytesExt, ByteOrder};
use crate::Result;

pub struct Decoder<R: std::io::Read + std::io::Seek> {
    reader: R,
}

impl<R: std::io::Read + std::io::Seek> Decoder<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    #[inline]
    pub fn read_uint(&mut self) -> Result<u64> {
        let u7_or_len = self.read_u8()?;
        if u7_or_len < 128 {
            return Ok(u7_or_len as u64);
        }
        let len = !u7_or_len + 1;
        self.fast_get_uint_be(len as usize)
    }

    fn fast_get_uint_be(&mut self, nbytes: usize) -> Result<u64> {
        let mut buf = vec![0; nbytes];
        self.reader.read_exact(&mut buf)?;
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

    pub fn parse(&mut self) -> Result<()> {
        loop {
            let msg_length = self.read_uint()? as usize;
            println!("msg_length: {}", &msg_length);
            
            let mut payload = vec![0; msg_length];
            self.reader.read_exact(&mut payload)?;

            let mut msg_decoder = Decoder::new(std::io::Cursor::new(payload));
            let type_id = msg_decoder.read_int()?;
            println!("type_id: {}", type_id);

            if type_id < 0 {
                let num = msg_decoder.read_int()?;
                let num0 = msg_decoder.read_int()?;
                let num1 = msg_decoder.read_int()?;
                let num2 = msg_decoder.read_int()?;
                let num3 = msg_decoder.read_int()?;
                let num4 = msg_decoder.read_int()?;
                println!("num: {} {} {} {} {} {}", num, num0, num1, num2, num3, num4);
            }
            // let len = msg_reader.read_varint::<u64>()?;
            // let len2 = msg_reader.read_varint::<u64>()?;

            // let len3 = msg_reader.read_varint::<u64>()?;
            // let len4 = msg_reader.read_varint::<u64>()?;

            // let len5 = msg_reader.read_varint::<u64>()?;
            // let len6 = msg_reader.read_varint::<u64>()?;

            // let len7 = msg_reader.read_varint::<u64>()?;
            // let len8 = msg_reader.read_varint::<u64>()?;

        }
        Ok(())
    }
}
