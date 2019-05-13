extern crate byteorder;

use crate::varint;
use byteorder::{ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Result, Write};

type Type = u8;
const TYPE_INT: Type = 'i' as u8;
const TYPE_UINT: Type = 'u' as u8;
const TYPE_FLOAT: Type = 'f' as u8;
const TYPE_BYTES: Type = 'b' as u8;
const TYPE_STRING: Type = 's' as u8;
const TYPE_LIST: Type = 'l' as u8;
const TYPE_MAP: Type = 'm' as u8;
const CONTAINER_CAPACITY: usize = 255;

pub type List = Vec<Typed>;
pub type Map = HashMap<String, Typed>;

#[derive(Debug, Clone, PartialEq)]
pub enum Typed {
    Int(i64),
    Uint(u64),
    Float(f64),
    Bytes(Vec<u8>),
    String(String),
    List(List),
    Map(Map),
}

pub trait CodecReadExt: ReadBytesExt + varint::VarintReadExt {
    fn read_sized(&mut self) -> Result<Vec<u8>> {
        let sz = self.read_uvarint()?;
        println!("{}", sz);
        let mut buf = vec![0u8; sz as usize];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_typed(&mut self) -> Result<Typed> {
        let t = self.read_u8()?;
        match t {
            TYPE_INT => {
                let n = self.read_varint()?;
                Ok(Typed::Int(n))
            }
            TYPE_UINT => {
                let un = self.read_uvarint()?;
                Ok(Typed::Uint(un))
            }
            TYPE_FLOAT => {
                let un = self.read_uvarint()?;
                Ok(Typed::Float(f64::from_bits(un)))
            }
            TYPE_BYTES => {
                let bs = self.read_sized()?;
                Ok(Typed::Bytes(bs))
            }
            TYPE_STRING => {
                let buf = self.read_sized()?;
                let s = String::from_utf8_lossy(&buf).to_string();
                Ok(Typed::String(s))
            }
            TYPE_LIST => {
                let l = self.read_list()?;
                Ok(Typed::List(l))
            }
            TYPE_MAP => {
                let m = self.read_map()?;
                Ok(Typed::Map(m))
            }
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("unknown type: '{}'", t),
            )),
        }
    }

    fn read_list(&mut self) -> Result<List> {
        let nelem = self.read_u8()?;
        let mut l = List::with_capacity(nelem as usize);
        if nelem == 0 {
            return Ok(l);
        }

        for _ in 0..nelem {
            let e = self.read_typed()?;
            l.push(e);
        }

        Ok(l)
    }

    fn read_map(&mut self) -> Result<Map> {
        let nelem = self.read_u8()?;
        let mut m = Map::new();
        if nelem == 0 {
            return Ok(m);
        }

        for _ in 0..nelem {
            let k = self.read_sized()?;
            let k = String::from_utf8_lossy(&k).to_string();
            let v = self.read_typed()?;
            m.insert(k, v);
        }

        Ok(m)
    }
}

/// All types that implement `Read` get methods defined in `CodecReadExt`
/// for free.
impl<R: Read + ?Sized> CodecReadExt for R {}

pub trait CodecWriteExt: WriteBytesExt + varint::VarintWriteExt {
    fn write_sized(&mut self, buf: &[u8]) -> Result<()> {
        self.write_uvarint(buf.len() as u64)?;
        self.write_all(buf)
    }

    fn write_typed(&mut self, e: &Typed) -> Result<()> {
        match e {
            Typed::Int(n) => {
                self.write_u8(TYPE_INT)?;
                self.write_varint(*n)
            }
            Typed::Uint(un) => {
                self.write_u8(TYPE_UINT)?;
                self.write_uvarint(*un)
            }
            Typed::Float(f) => {
                self.write_u8(TYPE_FLOAT)?;
                self.write_uvarint(f.to_bits())
            }
            Typed::Bytes(buf) => {
                self.write_u8(TYPE_BYTES)?;
                self.write_sized(&buf)
            }
            Typed::String(s) => {
                self.write_u8(TYPE_STRING)?;
                let buf = Vec::from(s.clone());
                self.write_sized(&buf)
            }
            Typed::List(l) => {
                self.write_u8(TYPE_LIST)?;
                self.write_list(l)
            }
            Typed::Map(m) => {
                self.write_u8(TYPE_MAP)?;
                self.write_map(m)
            }
        }
    }

    fn write_list(&mut self, l: &List) -> Result<()> {
        let nelem = l.len();
        if nelem >= CONTAINER_CAPACITY {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("containers can only contain 255 elements"),
            ));
        }

        self.write_u8(nelem as u8)?;
        for e in l.iter() {
            self.write_typed(e)?;
        }
        Ok(())
    }

    fn write_map(&mut self, m: &Map) -> Result<()> {
        let nelem = m.len();
        if nelem >= CONTAINER_CAPACITY {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("containers can only contain 255 elements"),
            ));
        }

        self.write_u8(nelem as u8)?;
        for (k, v) in m.iter() {
            let buf = Vec::from(k.clone());
            self.write_sized(&buf)?;
            self.write_typed(v)?;
        }
        Ok(())
    }
}

/// All types that implement `Write` get methods defined in `CodecWriteExt`
/// for free.
impl<W: Write + ?Sized> CodecWriteExt for W {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::io::prelude::*;

    #[test]
    fn sized_bytes() {
        let mut buf = io::Cursor::new(Vec::new());
        for s in ["a", "ab", "abc", "abcd", "abcde", "abcdef"].into_iter() {
            buf.seek(io::SeekFrom::Start(0)).unwrap();
            assert!(buf.write_sized(s.as_bytes()).is_ok());
            buf.seek(io::SeekFrom::Start(0)).unwrap();
            match buf.read_sized() {
                Ok(bs) => assert_eq!(&bs[..], s.as_bytes()),
                Err(err) => assert!(false, "{}", err),
            }
        }
    }

    #[test]
    fn list() {
        let mut buf = io::Cursor::new(Vec::new());
        let mut m = Map::new();
        m.insert(String::from("hi"), Typed::String(String::from("hello")));
        let l = vec![
            Typed::Int(0),
            Typed::Int(i8::max_value() as i64),
            Typed::Int(i16::max_value() as i64),
            Typed::Int(i32::max_value() as i64),
            Typed::Int(i64::max_value()),
            Typed::Uint(u8::max_value() as u64),
            Typed::Uint(u16::max_value() as u64),
            Typed::Uint(u32::max_value() as u64),
            Typed::Uint(u64::max_value()),
            Typed::Float(0.0),
            Typed::Float(12345.1231445),
            Typed::Bytes(vec![0u8, 1u8, 128u8, 255u8]),
            Typed::String(String::from("")),
            Typed::String(String::from("hello")),
            Typed::String(String::from("超")),
            Typed::List(vec![
                Typed::Int(123),
                Typed::Uint(456),
                Typed::Float(789.123),
                Typed::Bytes(vec![]),
                Typed::String(String::from("dumb")),
                Typed::Map(m.clone()),
            ]),
            Typed::Map(m.clone()),
        ];

        assert!(buf.write_list(&l).is_ok());
        buf.seek(io::SeekFrom::Start(0)).unwrap();
        match buf.read_list() {
            Ok(lread) => assert_eq!(lread, l),
            Err(err) => assert!(false, "{}", err),
        }
    }

    #[test]
    fn map() {
        let mut buf = io::Cursor::new(Vec::new());
        let mut m = Map::new();
        m.insert(String::from("0"), Typed::Int(0));
        m.insert(String::from("01"), Typed::Int(i8::max_value() as i64));
        m.insert(String::from("012"), Typed::Int(i16::max_value() as i64));
        m.insert(String::from("0123"), Typed::Int(i32::max_value() as i64));
        m.insert(String::from("01234"), Typed::Int(i64::max_value()));
        m.insert(String::from("012345"), Typed::Uint(u8::max_value() as u64));
        m.insert(
            String::from("0123456"),
            Typed::Uint(u16::max_value() as u64),
        );
        m.insert(
            String::from("01234567"),
            Typed::Uint(u32::max_value() as u64),
        );
        m.insert(String::from("012345678"), Typed::Uint(u64::max_value()));
        m.insert(String::from("0123456789"), Typed::Float(0.0));
        m.insert(String::from("9012345678"), Typed::Float(54321.54321));
        m.insert(
            String::from("8901234567"),
            Typed::Bytes(vec![0u8, 1u8, 128u8, 255u8]),
        );
        m.insert(String::from("7890123456"), Typed::String(String::from("")));
        m.insert(
            String::from("6789012345"),
            Typed::String(String::from("hello")),
        );
        m.insert(
            String::from("5678901234"),
            Typed::String(String::from("超")),
        );
        let mut m2 = Map::new();
        m2.insert(String::from("0sa"), Typed::Int(0));
        m2.insert(
            String::from("012sfdasf345"),
            Typed::Uint(u8::max_value() as u64),
        );
        m2.insert(String::from("901234567sas8"), Typed::Float(54321.54321));
        m2.insert(
            String::from("8901234lj567"),
            Typed::Bytes(vec![0u8, 1u8, 128u8, 255u8]),
        );
        m2.insert(
            String::from("678901230945"),
            Typed::String(String::from("world")),
        );
        m.insert(
            String::from("list"),
            Typed::List(vec![
                Typed::Int(123),
                Typed::Uint(456),
                Typed::Float(789.123),
                Typed::Bytes(vec![]),
                Typed::String(String::from("dumb")),
                Typed::Map(m2.clone()),
            ]),
        );
        m.insert(String::from("nested-map"), Typed::Map(m2.clone()));

        assert!(buf.write_map(&m).is_ok());
        buf.seek(io::SeekFrom::Start(0)).unwrap();
        match buf.read_map() {
            Ok(mread) => assert_eq!(mread, m),
            Err(err) => assert!(false, "{}", err),
        }
    }
}
