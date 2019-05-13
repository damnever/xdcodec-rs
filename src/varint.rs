use std::io::{Error, ErrorKind, Read, Result, Write};

pub const MAX_VARINT_LEN: usize = 10;

/// VarintReadExt extends `Read` trait with signed varint support.
pub trait VarintReadExt: Read {
    fn read_uvarint(&mut self) -> Result<u64> {
        let mut x = 0u64;
        let mut s = 0u64;
        let mut i = 0;
        let mut buf = [0u8; 1];

        loop {
            self.read_exact(&mut buf)?;
            let byte = buf[0] as u8;
            if byte < 0x80 {
                if i > 9 || i == 9 && byte > 1 {
                    return Err(Error::from(ErrorKind::InvalidData));
                }
                return Ok(x | (byte as u64) << s);
            }
            x |= ((byte & 0x7f) as u64) << s;
            s += 7;
            i += 1;
        }
    }

    fn read_varint(&mut self) -> Result<i64> {
        let un = self.read_uvarint()?;
        let mut n = (un >> 1) as i64;
        if un & 1 != 0 {
            n = !n;
        }
        Ok(n)
    }
}

/// All types that implement `Read` get methods defined in `VarintReadExt`
/// for free.
impl<R: Read + ?Sized> VarintReadExt for R {}

/// VarintWriteExt extends `Write` trait with unsigned varint support.
pub trait VarintWriteExt: Write {
    fn write_uvarint(&mut self, un: u64) -> Result<()> {
        let mut buf = [0u8; MAX_VARINT_LEN];
        let mut un = un;
        let mut i = 0usize;

        while un >= 0x80 {
            buf[i] = (un as u8) | 0x80;
            un >>= 7;
            i += 1;
        }
        buf[i] = un as u8;
        i += 1;
        self.write_all(&buf[..i])
    }

    fn write_varint(&mut self, n: i64) -> Result<()> {
        let mut un = (n as u64) << 1;
        if n < 0 {
            un = !un;
        }
        self.write_uvarint(un)
    }
}

/// All types that implement `Write` get methods defined in `VarintWriteExt`
/// for free.
impl<W: Write + ?Sized> VarintWriteExt for W {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::io::prelude::*;

    #[test]
    fn uvarint() {
        let mut buf = io::Cursor::new(vec![0u8; MAX_VARINT_LEN]);
        for n in [
            0u64,
            123456789,
            i8::max_value() as u64,
            u8::max_value() as u64,
            i16::max_value() as u64,
            u16::max_value() as u64,
            i32::max_value() as u64,
            u32::max_value() as u64,
            u64::max_value(),
        ]
        .into_iter()
        {
            buf.seek(io::SeekFrom::Start(0)).unwrap();
            assert!(buf.write_uvarint(*n).is_ok());
            buf.seek(io::SeekFrom::Start(0)).unwrap();
            match buf.read_uvarint() {
                Ok(nread) => assert_eq!(nread, *n),
                Err(err) => assert!(false, "{}", err),
            }
        }
    }

    #[test]
    fn varint() {
        let mut buf = io::Cursor::new(vec![0u8; MAX_VARINT_LEN]);
        for n in [
            0i64,
            123456789,
            i8::max_value() as i64,
            u8::max_value() as i64,
            i16::max_value() as i64,
            u16::max_value() as i64,
            i32::max_value() as i64,
            u32::max_value() as i64,
            i64::max_value(),
        ]
        .into_iter()
        {
            buf.seek(io::SeekFrom::Start(0)).unwrap();
            assert!(buf.write_varint(*n).is_ok());
            buf.seek(io::SeekFrom::Start(0)).unwrap();
            match buf.read_varint() {
                Ok(nread) => assert_eq!(nread, *n),
                Err(err) => assert!(false, "{}", err),
            }
        }
    }
}
