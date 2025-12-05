use std::io::Write;
use crate::Result;

pub struct Encoder<W: Write> {
    writer: W,
}

impl<W: Write> Encoder<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Writes a single byte.
    pub fn write_u8(&mut self, v: u8) -> Result<()> {
        self.writer.write_all(&[v])?;
        Ok(())
    }

    /// Writes an unsigned integer using gob's variable-length encoding.
    /// Tiny values (< 128) are written as a single byte.
    /// Larger values are written as a length prefix (inverted count) followed by the bytes in big-endian order.
    pub fn write_uint(&mut self, v: u64) -> Result<()> {
        if v < 128 {
            self.write_u8(v as u8)?;
            return Ok(());
        }

        let mut buf = [0u8; 9]; // Max 8 bytes for u64 + potential length logic
        let mut n = 0;
        let mut temp = v;
        while temp > 0 {
            n += 1;
            temp >>= 8;
        }

        // The length prefix logic:
        // n is number of bytes. 
        // We write !(n-1) as the prefix.
        let len_byte = !(n as u8 - 1); 
        self.write_u8(len_byte)?;
        
        // Write bytes big-endian
        let mut temp = v;
        for i in 0..n {
             buf[n - 1 - i] = (temp & 0xFF) as u8;
             temp >>= 8;
        }
        self.writer.write_all(&buf[0..n])?;
        Ok(())
    }

    /// Writes a signed integer.
    /// Signed integers are zigzag-encoded (or similar) into an unsigned integer, then written.
    pub fn write_int(&mut self, v: i64) -> Result<()> {
        let u: u64;
        if v < 0 {
            u = ((!v as u64) << 1) | 1;
        } else {
            u = (v as u64) << 1;
        }
        self.write_uint(u)
    }

    /// Writes a floating point number.
    /// Floats are bit-reversed and then encoded as uints.
    pub fn write_float(&mut self, v: f64) -> Result<()> {
        let bits = v.to_bits();
        let swapped = bits.swap_bytes();
        self.write_uint(swapped)
    }

    /// Writes a boolean value.
    pub fn write_bool(&mut self, v: bool) -> Result<()> {
        if v {
            self.write_uint(1)
        } else {
            self.write_uint(0)
        }
    }

    /// Writes a byte slice.
    /// Encoded as length (uint) followed by raw bytes.
    pub fn write_bytes(&mut self, v: &[u8]) -> Result<()> {
        self.write_uint(v.len() as u64)?;
        self.writer.write_all(v)?;
        Ok(())
    }

    /// Writes a string.
    /// Encoded as a byte slice.
    pub fn write_string(&mut self, v: &str) -> Result<()> {
        self.write_bytes(v.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::Decoder;
    use std::io::Cursor;

    #[test]
    fn test_uint_encoding() {
        let tests = vec![
            (0, vec![0]),
            (127, vec![127]),
            (128, vec![255, 128]),
            (256, vec![254, 1, 0]),
        ];

        for (val, expected) in tests {
            let mut buf = Vec::new();
            let mut enc = Encoder::new(&mut buf);
            enc.write_uint(val).unwrap();
            assert_eq!(buf, expected, "Failed encoding {}", val);

            let mut cursor = Cursor::new(buf);
            let mut dec = Decoder::new(&mut cursor);
            let decoded = dec.read_uint().unwrap();
            assert_eq!(decoded, val, "Failed decoding {}", val);
        }
    }

    #[test]
    fn test_int_encoding() {
        let tests = vec![
            (0, 0),
            (-1, -1),
            (1, 1),
            (-128, -128),
            (128, 128),
        ];

        for (val, _) in tests {
            let mut buf = Vec::new();
            let mut enc = Encoder::new(&mut buf);
            enc.write_int(val).unwrap();

            let mut cursor = Cursor::new(buf);
            let mut dec = Decoder::new(&mut cursor);
            let decoded = dec.read_int().unwrap();
            assert_eq!(decoded, val, "Failed decoding {}", val);
        }
    }
    
    #[test]
    fn test_string_encoding() {
        let val = "Hello World";
        let mut buf = Vec::new();
        let mut enc = Encoder::new(&mut buf);
        enc.write_string(val).unwrap();

        let mut cursor = Cursor::new(buf);
        let mut dec = Decoder::new(&mut cursor);
        let decoded = dec.read_string().unwrap();
        assert_eq!(decoded, val);
    }
}
