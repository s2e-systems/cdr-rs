//! Deserializing CDR into Rust data types.

use std::{self, io::Read};

use serde::de::{self, IntoDeserializer};

use crate::error::{Error, Result};
use crate::size::{Infinite, SizeLimit};

use crate::{Endianness, RepresentationFormat};

/// A deserializer that reads bytes from a buffer.
pub struct Deserializer<R, S> {
    reader: R,
    size_limit: S,
    pos: usize,
    endianness: Endianness,
}

impl<R, S> Deserializer<R, S>
where
    R: Read,
    S: SizeLimit,
{
    pub fn new(reader: R, representation_format: &RepresentationFormat, size_limit: S) -> Self {
        let endianness = representation_format.endianness();

        Self {
            reader,
            size_limit,
            pos: 0,
            endianness,
        }
    }

    pub fn set_representation_format(&mut self, representation_format: &RepresentationFormat) {
        self.endianness = representation_format.endianness();
    }

    fn read_padding_of<T>(&mut self) -> Result<()> {
        // Calculate the required padding to align with 1-byte, 2-byte, 4-byte, 8-byte boundaries
        // Instead of using the slow modulo operation '%', the faster bit-masking is used
        let alignment = std::mem::size_of::<T>();
        let rem_mask = alignment - 1; // mask like 0x0, 0x1, 0x3, 0x7
        let mut padding: [u8; 8] = [0; 8];
        match (self.pos as usize) & rem_mask {
            0 => Ok(()),
            n @ 1..=7 => {
                let amt = alignment - n;
                self.read_size(amt)?;
                self.reader
                    .read_exact(&mut padding[..amt])
                    .map_err(Into::into)
            }
            _ => unreachable!(),
        }
    }

    fn read_size(&mut self, size: usize) -> Result<()> {
        self.pos += size;
        self.size_limit.add(size)
    }

    fn read_size_of<T>(&mut self) -> Result<()> {
        self.read_size(std::mem::size_of::<T>())
    }

    fn read_string(&mut self) -> Result<String> {
        String::from_utf8(self.read_vec().map(|mut v| {
            v.pop(); // removes a terminating null character
            v
        })?)
        .map_err(|e| Error::InvalidUtf8Encoding(e.utf8_error()))
    }

    fn read_vec(&mut self) -> Result<Vec<u8>> {
        let len: u32 = de::Deserialize::deserialize(&mut *self)?;
        let mut buf = Vec::with_capacity(len as usize);
        unsafe { buf.set_len(len as usize) }
        self.read_size(u64::from(len) as usize)?;
        self.reader.read_exact(&mut buf[..])?;
        Ok(buf)
    }

    pub(crate) fn reset_pos(&mut self) {
        self.pos = 0;
    }
}

impl<'de, 'a, R, S> de::Deserializer<'de> for &'a mut Deserializer<R, S>
where
    R: Read,
    S: SizeLimit,
{
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::DeserializeAnyNotSupported)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let value: u8 = de::Deserialize::deserialize(self)?;
        match value {
            1 => visitor.visit_bool(true),
            0 => visitor.visit_bool(false),
            value => Err(Error::InvalidBoolEncoding(value)),
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_size_of::<u8>()?;

        let mut buf: [u8; 1] = [0; 1];
        self.reader.read_exact(&mut buf)?;
        visitor.visit_u8(u8::from_ne_bytes(buf))
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<u16>()?;
        self.read_size_of::<u16>()?;

        let mut buf: [u8; 2] = [0; 2];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => u16::from_be_bytes(buf),
            Endianness::LittleEndian => u16::from_le_bytes(buf),
        };
        visitor.visit_u16(v)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<u32>()?;
        self.read_size_of::<u32>()?;

        let mut buf: [u8; 4] = [0; 4];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => u32::from_be_bytes(buf),
            Endianness::LittleEndian => u32::from_le_bytes(buf),
        };

        visitor.visit_u32(v)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<u64>()?;
        self.read_size_of::<u64>()?;

        let mut buf: [u8; 8] = [0; 8];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => u64::from_be_bytes(buf),
            Endianness::LittleEndian => u64::from_le_bytes(buf),
        };

        visitor.visit_u64(v)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_size_of::<i8>()?;

        let mut buf: [u8; 1] = [0; 1];
        self.reader.read_exact(&mut buf)?;

        visitor.visit_i8(i8::from_ne_bytes(buf))
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<i16>()?;
        self.read_size_of::<i16>()?;

        let mut buf: [u8; 2] = [0; 2];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => i16::from_be_bytes(buf),
            Endianness::LittleEndian => i16::from_le_bytes(buf),
        };

        visitor.visit_i16(v)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<i32>()?;
        self.read_size_of::<i32>()?;

        let mut buf: [u8; 4] = [0; 4];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => i32::from_be_bytes(buf),
            Endianness::LittleEndian => i32::from_le_bytes(buf),
        };

        visitor.visit_i32(v)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<i64>()?;
        self.read_size_of::<i64>()?;

        let mut buf: [u8; 8] = [0; 8];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => i64::from_be_bytes(buf),
            Endianness::LittleEndian => i64::from_le_bytes(buf),
        };

        visitor.visit_i64(v)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<f32>()?;
        self.read_size_of::<f32>()?;

        let mut buf: [u8; 4] = [0; 4];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => f32::from_be_bytes(buf),
            Endianness::LittleEndian => f32::from_le_bytes(buf),
        };

        visitor.visit_f32(v)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.read_padding_of::<f64>()?;
        self.read_size_of::<f64>()?;

        let mut buf: [u8; 8] = [0; 8];
        self.reader.read_exact(&mut buf)?;
        let v = match self.endianness {
            Endianness::BigEndian => f64::from_be_bytes(buf),
            Endianness::LittleEndian => f64::from_le_bytes(buf),
        };

        visitor.visit_f64(v)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let mut buf = [0u8; 4];
        self.reader.read_exact(&mut buf[..1])?;

        let width = utf8_char_width(buf[0]);
        if width != 1 {
            Err(Error::InvalidCharEncoding)
        } else {
            self.read_size(width)?;
            visitor.visit_char(buf[0] as char)
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_str(&self.read_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.read_string()?)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_bytes(&self.read_vec()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_byte_buf(self.read_vec()?)
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::TypeNotSupported)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let len: u32 = de::Deserialize::deserialize(&mut *self)?;
        self.deserialize_tuple(len as usize, visitor)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        struct Access<'a, R: 'a, S: 'a>
        where
            R: Read,
            S: SizeLimit,
        {
            deserializer: &'a mut Deserializer<R, S>,
            len: usize,
        }

        impl<'de, 'a, R: 'a, S> de::SeqAccess<'de> for Access<'a, R, S>
        where
            R: Read,
            S: SizeLimit,
        {
            type Error = Error;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
            where
                T: de::DeserializeSeed<'de>,
            {
                if self.len > 0 {
                    self.len -= 1;
                    let value = de::DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
                    Ok(Some(value))
                } else {
                    Ok(None)
                }
            }

            fn size_hint(&self) -> Option<usize> {
                Some(self.len)
            }
        }

        visitor.visit_seq(Access {
            deserializer: self,
            len,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::TypeNotSupported)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_tuple(fields.len(), visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        impl<'de, 'a, R: 'a, S> de::EnumAccess<'de> for &'a mut Deserializer<R, S>
        where
            R: Read,
            S: SizeLimit,
        {
            type Error = Error;
            type Variant = Self;

            fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
            where
                V: de::DeserializeSeed<'de>,
            {
                let idx: u32 = de::Deserialize::deserialize(&mut *self)?;
                let val: Result<_> = seed.deserialize(idx.into_deserializer());
                Ok((val?, self))
            }
        }

        visitor.visit_enum(self)
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::TypeNotSupported)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::TypeNotSupported)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

impl<'de, 'a, R, S> de::VariantAccess<'de> for &'a mut Deserializer<R, S>
where
    R: Read,
    S: SizeLimit,
{
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        de::DeserializeSeed::deserialize(seed, self)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_tuple(self, len, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_tuple(self, fields.len(), visitor)
    }
}

#[inline]
fn utf8_char_width(first_byte: u8) -> usize {
    UTF8_CHAR_WIDTH[first_byte as usize] as usize
}

// https://tools.ietf.org/html/rfc3629
const UTF8_CHAR_WIDTH: &[u8; 256] = &[
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x1F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x3F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x5F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x7F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0x9F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0xBF
    0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, //
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // 0xDF
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, // 0xEF
    4, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0xFF
];

/// Deserializes a slice of bytes into an object.
pub fn deserialize_data<'de, T>(
    bytes: &[u8],
    representation_format: RepresentationFormat,
) -> Result<T>
where
    T: de::Deserialize<'de>,
{
    deserialize_data_from(bytes, representation_format, Infinite)
}

/// Deserializes an object directly from a `Read`.
pub fn deserialize_data_from<'de, R, T, S>(
    reader: R,
    representation_format: RepresentationFormat,
    size_limit: S,
) -> Result<T>
where
    R: Read,
    T: de::Deserialize<'de>,
    S: SizeLimit,
{
    let mut deserializer = Deserializer::new(reader, &representation_format, size_limit);
    de::Deserialize::deserialize(&mut deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_octet() {
        assert_eq!(
            deserialize_data::<u8>(&vec![0x20], RepresentationFormat::CdrBe).unwrap(),
            32u8
        );
        assert_eq!(
            deserialize_data::<u8>(&vec![0x20], RepresentationFormat::CdrLe).unwrap(),
            32u8
        );
    }

    #[test]
    fn deserialize_char() {
        assert_eq!(
            deserialize_data::<char>(&vec![0x5a], RepresentationFormat::CdrBe).unwrap(),
            'Z'
        );
        assert_eq!(
            deserialize_data::<char>(&vec![0x5a], RepresentationFormat::CdrLe).unwrap(),
            'Z'
        );
    }

    #[test]
    fn deserialize_ushort() {
        assert_eq!(
            deserialize_data::<u16>(&vec![0xff, 0xdc], RepresentationFormat::CdrBe).unwrap(),
            65500u16
        );
        assert_eq!(
            deserialize_data::<u16>(&vec![0xdc, 0xff], RepresentationFormat::CdrLe).unwrap(),
            65500u16
        );
    }

    #[test]
    fn deserialize_short() {
        assert_eq!(
            deserialize_data::<i16>(&vec![0x80, 0x44], RepresentationFormat::CdrBe).unwrap(),
            -32700i16
        );
        assert_eq!(
            deserialize_data::<i16>(&vec![0x44, 0x80], RepresentationFormat::CdrLe).unwrap(),
            -32700i16
        );
    }

    #[test]
    fn deserialize_ulong() {
        assert_eq!(
            deserialize_data::<u32>(&vec![0xff, 0xff, 0xff, 0xa0], RepresentationFormat::CdrBe)
                .unwrap(),
            4294967200u32
        );
        assert_eq!(
            deserialize_data::<u32>(&vec![0xa0, 0xff, 0xff, 0xff], RepresentationFormat::CdrLe)
                .unwrap(),
            4294967200u32
        );
    }

    #[test]
    fn deserialize_long() {
        assert_eq!(
            deserialize_data::<i32>(&vec![0x80, 0x00, 0x00, 0x30], RepresentationFormat::CdrBe)
                .unwrap(),
            -2147483600i32
        );
        assert_eq!(
            deserialize_data::<i32>(&vec![0x30, 0x00, 0x00, 0x80], RepresentationFormat::CdrLe)
                .unwrap(),
            -2147483600i32
        );
    }

    #[test]
    fn deserialize_ulonglong() {
        assert_eq!(
            deserialize_data::<u64>(
                &vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            18446744073709551600u64
        );
        assert_eq!(
            deserialize_data::<u64>(
                &vec![0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            18446744073709551600u64
        );
    }

    #[test]
    fn deserialize_longlong() {
        assert_eq!(
            deserialize_data::<i64>(
                &vec![0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x40],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            -9223372036800i64
        );
        assert_eq!(
            deserialize_data::<i64>(
                &vec![0x40, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            -9223372036800i64
        );
    }

    #[test]
    fn deserialize_float() {
        assert_eq!(
            deserialize_data::<f32>(&vec![0x00, 0x80, 0x00, 0x00], RepresentationFormat::CdrBe)
                .unwrap(),
            std::f32::MIN_POSITIVE
        );
        assert_eq!(
            deserialize_data::<f32>(&vec![0x00, 0x00, 0x80, 0x00], RepresentationFormat::CdrLe)
                .unwrap(),
            std::f32::MIN_POSITIVE
        );
    }

    #[test]
    fn deserialize_double() {
        assert_eq!(
            deserialize_data::<f64>(
                &vec![0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            std::f64::MIN_POSITIVE
        );
        assert_eq!(
            deserialize_data::<f64>(
                &vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            std::f64::MIN_POSITIVE
        );
    }

    #[test]
    fn deserialize_bool() {
        assert_eq!(
            deserialize_data::<bool>(&vec![0x01], RepresentationFormat::CdrBe).unwrap(),
            true
        );
        assert_eq!(
            deserialize_data::<bool>(&vec![0x01], RepresentationFormat::CdrLe).unwrap(),
            true
        );
    }

    #[test]
    fn deserialize_string() {
        assert_eq!(
            deserialize_data::<String>(
                &vec![
                    0x00, 0x00, 0x00, 0x1e, 0x48, 0x6f, 0x6c, 0x61, 0x20, 0x61, 0x20, 0x74, 0x6f,
                    0x64, 0x6f, 0x73, 0x2c, 0x20, 0x65, 0x73, 0x74, 0x6f, 0x20, 0x65, 0x73, 0x20,
                    0x75, 0x6e, 0x20, 0x74, 0x65, 0x73, 0x74, 0x00,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            String::from("Hola a todos, esto es un test")
        );
        assert_eq!(
            deserialize_data::<String>(
                &vec![
                    0x1e, 0x00, 0x00, 0x00, 0x48, 0x6f, 0x6c, 0x61, 0x20, 0x61, 0x20, 0x74, 0x6f,
                    0x64, 0x6f, 0x73, 0x2c, 0x20, 0x65, 0x73, 0x74, 0x6f, 0x20, 0x65, 0x73, 0x20,
                    0x75, 0x6e, 0x20, 0x74, 0x65, 0x73, 0x74, 0x00,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            String::from("Hola a todos, esto es un test")
        );
    }

    #[test]
    fn deserialize_empty_string() {
        assert_eq!(
            deserialize_data::<String>(
                &vec![0x00, 0x00, 0x00, 0x01, 0x00],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            ""
        );
        assert_eq!(
            deserialize_data::<String>(
                &vec![0x01, 0x00, 0x00, 0x00, 0x00],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            ""
        );
    }

    #[test]
    fn deserialize_octet_array() {
        assert_eq!(
            deserialize_data::<[u8; 5]>(
                &vec![0x01, 0x02, 0x03, 0x04, 0x05],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            [1u8, 2, 3, 4, 5]
        );
        assert_eq!(
            deserialize_data::<[u8; 5]>(
                &vec![0x01, 0x02, 0x03, 0x04, 0x05],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            [1u8, 2, 3, 4, 5]
        );
    }

    #[test]
    fn deserialize_char_array() {
        let v = ['A', 'B', 'C', 'D', 'E'];

        assert_eq!(
            deserialize_data::<[char; 5]>(
                &vec![0x41, 0x42, 0x43, 0x44, 0x45],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[char; 5]>(
                &vec![0x41, 0x42, 0x43, 0x44, 0x45],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn serialize_ushort_array() {
        let v = [65500u16, 65501, 65502, 65503, 65504];
        assert_eq!(
            deserialize_data::<[u16; 5]>(
                &vec![0xff, 0xdc, 0xff, 0xdd, 0xff, 0xde, 0xff, 0xdf, 0xff, 0xe0],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[u16; 5]>(
                &vec![0xdc, 0xff, 0xdd, 0xff, 0xde, 0xff, 0xdf, 0xff, 0xe0, 0xff],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_short_array() {
        let v = [-32700i16, -32701, -32702, -32703, -32704];
        assert_eq!(
            deserialize_data::<[i16; 5]>(
                &vec![0x80, 0x44, 0x80, 0x43, 0x80, 0x42, 0x80, 0x41, 0x80, 0x40],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[i16; 5]>(
                &vec![0x44, 0x80, 0x43, 0x80, 0x42, 0x80, 0x41, 0x80, 0x40, 0x80],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_ulong_array() {
        let v = [
            4294967200u32,
            4294967201,
            4294967202,
            4294967203,
            4294967204,
        ];
        assert_eq!(
            deserialize_data::<[u32; 5]>(
                &vec![
                    0xff, 0xff, 0xff, 0xa0, 0xff, 0xff, 0xff, 0xa1, 0xff, 0xff, 0xff, 0xa2, 0xff,
                    0xff, 0xff, 0xa3, 0xff, 0xff, 0xff, 0xa4,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[u32; 5]>(
                &vec![
                    0xa0, 0xff, 0xff, 0xff, 0xa1, 0xff, 0xff, 0xff, 0xa2, 0xff, 0xff, 0xff, 0xa3,
                    0xff, 0xff, 0xff, 0xa4, 0xff, 0xff, 0xff,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_long_array() {
        let v = [
            -2147483600,
            -2147483601,
            -2147483602,
            -2147483603,
            -2147483604,
        ];
        assert_eq!(
            deserialize_data::<[i32; 5]>(
                &vec![
                    0x80, 0x00, 0x00, 0x30, //
                    0x80, 0x00, 0x00, 0x2f, //
                    0x80, 0x00, 0x00, 0x2e, //
                    0x80, 0x00, 0x00, 0x2d, //
                    0x80, 0x00, 0x00, 0x2c,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[i32; 5]>(
                &vec![
                    0x30, 0x00, 0x00, 0x80, //
                    0x2f, 0x00, 0x00, 0x80, //
                    0x2e, 0x00, 0x00, 0x80, //
                    0x2d, 0x00, 0x00, 0x80, //
                    0x2c, 0x00, 0x00, 0x80,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_ulonglong_array() {
        let v = [
            18446744073709551600u64,
            18446744073709551601,
            18446744073709551602,
            18446744073709551603,
            18446744073709551604,
        ];
        assert_eq!(
            deserialize_data::<[u64; 5]>(
                &vec![
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xf1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf2, 0xff, 0xff,
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xf3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xf4,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );

        assert_eq!(
            deserialize_data::<[u64; 5]>(
                &vec![
                    0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf1, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff, 0xf2, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf3, 0xff,
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf4, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_longlong_array() {
        let v = [
            -9223372036800i64,
            -9223372036801,
            -9223372036802,
            -9223372036803,
            -9223372036804,
        ];
        assert_eq!(
            deserialize_data::<[i64; 5]>(
                &vec![
                    0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x40, 0xff, 0xff, 0xf7, 0x9c, 0x84,
                    0x2f, 0xa5, 0x3f, 0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x3e, 0xff, 0xff,
                    0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x3d, 0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5,
                    0x3c,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[i64; 5]>(
                &vec![
                    0x40, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff, 0x3f, 0xa5, 0x2f, 0x84, 0x9c,
                    0xf7, 0xff, 0xff, 0x3e, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff, //
                    0x3d, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff, 0x3c, 0xa5, 0x2f, 0x84, 0x9c,
                    0xf7, 0xff, 0xff,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_float_array() {
        let f = std::f32::MIN_POSITIVE;

        let v = [f, f + 1., f + 2., f + 3., f + 4.];
        assert_eq!(
            deserialize_data::<[f32; 5]>(
                &vec![
                    0x00, 0x80, 0x00, 0x00, //
                    0x3f, 0x80, 0x00, 0x00, //
                    0x40, 0x00, 0x00, 0x00, //
                    0x40, 0x40, 0x00, 0x00, //
                    0x40, 0x80, 0x00, 0x00,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[f32; 5]>(
                &vec![
                    0x00, 0x00, 0x80, 0x00, //
                    0x00, 0x00, 0x80, 0x3f, //
                    0x00, 0x00, 0x00, 0x40, //
                    0x00, 0x00, 0x40, 0x40, //
                    0x00, 0x00, 0x80, 0x40,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_double_array() {
        let f = std::f64::MIN_POSITIVE;

        let v = [f, f + 1., f + 2., f + 3., f + 4.];
        assert_eq!(
            deserialize_data::<[f64; 5]>(
                &vec![
                    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                    0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                    0x40, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                    0x40, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[f64; 5]>(
                &vec![
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, //
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, //
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, //
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x40, //
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x40,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_bool_array() {
        let v = [true, false, true, false, true];
        assert_eq!(
            deserialize_data::<[bool; 5]>(
                &vec![0x01, 0x00, 0x01, 0x00, 0x01],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[bool; 5]>(
                &vec![0x01, 0x00, 0x01, 0x00, 0x01],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_string_array() {
        let v = ["HOLA", "ADIOS", "HELLO", "BYE", "GOODBYE"];
        assert_eq!(
            deserialize_data::<[String; 5]>(
                &vec![
                    0x00, 0x00, 0x00, 0x05, //
                    0x48, 0x4f, 0x4c, 0x41, 0x00, //
                    0x00, 0x00, 0x00, //
                    0x00, 0x00, 0x00, 0x06, //
                    0x41, 0x44, 0x49, 0x4f, 0x53, 0x00, //
                    0x00, 0x00, //
                    0x00, 0x00, 0x00, 0x06, //
                    0x48, 0x45, 0x4c, 0x4c, 0x4f, 0x00, //
                    0x00, 0x00, //
                    0x00, 0x00, 0x00, 0x04, //
                    0x42, 0x59, 0x45, 0x00, //
                    0x00, 0x00, 0x00, 0x08, //
                    0x47, 0x4f, 0x4f, 0x44, 0x42, 0x59, 0x45, 0x00,
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<[String; 5]>(
                &vec![
                    0x05, 0x00, 0x00, 0x00, //
                    0x48, 0x4f, 0x4c, 0x41, 0x00, //
                    0x00, 0x00, 0x00, //
                    0x06, 0x00, 0x00, 0x00, //
                    0x41, 0x44, 0x49, 0x4f, 0x53, 0x00, //
                    0x00, 0x00, //
                    0x06, 0x00, 0x00, 0x00, //
                    0x48, 0x45, 0x4c, 0x4c, 0x4f, 0x00, //
                    0x00, 0x00, //
                    0x04, 0x00, 0x00, 0x00, //
                    0x42, 0x59, 0x45, 0x00, //
                    0x08, 0x00, 0x00, 0x00, //
                    0x47, 0x4f, 0x4f, 0x44, 0x42, 0x59, 0x45, 0x00,
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_octet_sequence() {
        let v = vec![1u8, 2, 3, 4, 5];
        assert_eq!(
            deserialize_data::<Vec<u8>>(
                &vec![
                    0x00, 0x00, 0x00, 0x05, //
                    0x01, 0x02, 0x03, 0x04, 0x05
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<u8>>(
                &vec![
                    0x05, 0x00, 0x00, 0x00, //
                    0x01, 0x02, 0x03, 0x04, 0x05
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_char_sequence() {
        let v = vec!['A', 'B', 'C', 'D', 'E'];
        assert_eq!(
            deserialize_data::<Vec<char>>(
                &vec![
                    0x00, 0x00, 0x00, 0x05, //
                    0x41, 0x42, 0x43, 0x44, 0x45
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<char>>(
                &vec![
                    0x05, 0x00, 0x00, 0x00, //
                    0x41, 0x42, 0x43, 0x44, 0x45
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_ushort_sequence() {
        let v = vec![65500u16, 65501, 65502, 65503, 65504];
        assert_eq!(
            deserialize_data::<Vec<u16>>(
                &vec![
                    0x00, 0x00, 0x00, 0x05, //
                    0xff, 0xdc, //
                    0xff, 0xdd, //
                    0xff, 0xde, //
                    0xff, 0xdf, //
                    0xff, 0xe0
                ],
                RepresentationFormat::CdrBe
            )
            .unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<u16>>(
                &vec![
                    0x05, 0x00, 0x00, 0x00, //
                    0xdc, 0xff, //
                    0xdd, 0xff, //
                    0xde, 0xff, //
                    0xdf, 0xff, //
                    0xe0, 0xff
                ],
                RepresentationFormat::CdrLe
            )
            .unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_short_sequence() {
        let v = vec![-32700i16, -32701, -32702, -32703, -32704];
        assert_eq!(
            deserialize_data::<Vec<i16>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0x80, 0x44, //
                0x80, 0x43, //
                0x80, 0x42, //
                0x80, 0x41, //
                0x80, 0x40
            ], RepresentationFormat::CdrBe).unwrap(),
            v
            
        );
        assert_eq!(
            deserialize_data::<Vec<i16>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x44, 0x80, //
                0x43, 0x80, //
                0x42, 0x80, //
                0x41, 0x80, //
                0x40, 0x80
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_ulong_sequence() {
        let v = vec![
            4294967200u32,
            4294967201,
            4294967202,
            4294967203,
            4294967204,
        ];
        assert_eq!(
            deserialize_data::<Vec<u32>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0xff, 0xff, 0xff, 0xa0, //
                0xff, 0xff, 0xff, 0xa1, //
                0xff, 0xff, 0xff, 0xa2, //
                0xff, 0xff, 0xff, 0xa3, //
                0xff, 0xff, 0xff, 0xa4,
            ], RepresentationFormat::CdrBe).unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<u32>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0xa0, 0xff, 0xff, 0xff, //
                0xa1, 0xff, 0xff, 0xff, //
                0xa2, 0xff, 0xff, 0xff, //
                0xa3, 0xff, 0xff, 0xff, //
                0xa4, 0xff, 0xff, 0xff,
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_long_sequence() {
        let v = vec![
            -2147483600,
            -2147483601,
            -2147483602,
            -2147483603,
            -2147483604,
        ];
        assert_eq!(
            deserialize_data::<Vec<i32>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0x80, 0x00, 0x00, 0x30, //
                0x80, 0x00, 0x00, 0x2f, //
                0x80, 0x00, 0x00, 0x2e, //
                0x80, 0x00, 0x00, 0x2d, //
                0x80, 0x00, 0x00, 0x2c,
            ], RepresentationFormat::CdrBe).unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<i32>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x30, 0x00, 0x00, 0x80, //
                0x2f, 0x00, 0x00, 0x80, //
                0x2e, 0x00, 0x00, 0x80, //
                0x2d, 0x00, 0x00, 0x80, //
                0x2c, 0x00, 0x00, 0x80,
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_ulonglong_sequence() {
        let v = vec![
            18446744073709551600u64,
            18446744073709551601,
            18446744073709551602,
            18446744073709551603,
            18446744073709551604,
        ];
        assert_eq!(
            deserialize_data::<Vec<u64>>(& vec![
                0x00, 0x00, 0x00, 0x05, //
                0x00, 0x00, 0x00, 0x00, //
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, //
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf1, //
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf2, //
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf3, //
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf4,
            ], RepresentationFormat::CdrBe).unwrap(),
           v
        );
        assert_eq!(
            deserialize_data::<Vec<u64>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, //
                0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
                0xf1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
                0xf2, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
                0xf3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
                0xf4, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_longlong_sequence() {
        let v = vec![
            -9223372036800i64,
            -9223372036801,
            -9223372036802,
            -9223372036803,
            -9223372036804,
        ];
        assert_eq!(
            deserialize_data::<Vec<i64>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0x00, 0x00, 0x00, 0x00, //
                0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x40, //
                0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x3f, //
                0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x3e, //
                0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x3d, //
                0xff, 0xff, 0xf7, 0x9c, 0x84, 0x2f, 0xa5, 0x3c,
            ], RepresentationFormat::CdrBe).unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<i64>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, //
                0x40, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff, //
                0x3f, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff, //
                0x3e, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff, //
                0x3d, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff, //
                0x3c, 0xa5, 0x2f, 0x84, 0x9c, 0xf7, 0xff, 0xff,
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_float_sequence() {
        let f = std::f32::MIN_POSITIVE;

        let v = vec![f, f + 1., f + 2., f + 3., f + 4.];
        assert_eq!(
            deserialize_data::<Vec<f32>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0x00, 0x80, 0x00, 0x00, //
                0x3f, 0x80, 0x00, 0x00, //
                0x40, 0x00, 0x00, 0x00, //
                0x40, 0x40, 0x00, 0x00, //
                0x40, 0x80, 0x00, 0x00,
            ], RepresentationFormat::CdrBe).unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<f32>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x80, 0x00, //
                0x00, 0x00, 0x80, 0x3f, //
                0x00, 0x00, 0x00, 0x40, //
                0x00, 0x00, 0x40, 0x40, //
                0x00, 0x00, 0x80, 0x40,
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_double_sequence() {
        let f = std::f64::MIN_POSITIVE;

        let v = vec![f, f + 1., f + 2., f + 3., f + 4.];
        assert_eq!(
            deserialize_data::<Vec<f64>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0x00, 0x00, 0x00, 0x00, //
                0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x40, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x40, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ], RepresentationFormat::CdrBe).unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<f64>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x40, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x40,
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_bool_sequence() {
        let v = vec![true, false, true, false, true];
        assert_eq!(
            deserialize_data::<Vec<bool>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0x01, 0x00, 0x01, 0x00, 0x01
            ], RepresentationFormat::CdrBe).unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<bool>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x01, 0x00, 0x01, 0x00, 0x01
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }

    #[test]
    fn deserialize_string_sequence() {
        let v = vec!["HOLA", "ADIOS", "HELLO", "BYE", "GOODBYE"];
        assert_eq!(
            deserialize_data::<Vec<String>>(&vec![
                0x00, 0x00, 0x00, 0x05, //
                0x00, 0x00, 0x00, 0x05, //
                0x48, 0x4f, 0x4c, 0x41, 0x00, //
                0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x06, //
                0x41, 0x44, 0x49, 0x4f, 0x53, 0x00, //
                0x00, 0x00, //
                0x00, 0x00, 0x00, 0x06, //
                0x48, 0x45, 0x4c, 0x4c, 0x4f, 0x00, //
                0x00, 0x00, //
                0x00, 0x00, 0x00, 0x04, //
                0x42, 0x59, 0x45, 0x00, //
                0x00, 0x00, 0x00, 0x08, //
                0x47, 0x4f, 0x4f, 0x44, 0x42, 0x59, 0x45, 0x00,
            ], RepresentationFormat::CdrBe).unwrap(),
            v
        );
        assert_eq!(
            deserialize_data::<Vec<String>>(&vec![
                0x05, 0x00, 0x00, 0x00, //
                0x05, 0x00, 0x00, 0x00, //
                0x48, 0x4f, 0x4c, 0x41, 0x00, //
                0x00, 0x00, 0x00, //
                0x06, 0x00, 0x00, 0x00, //
                0x41, 0x44, 0x49, 0x4f, 0x53, 0x00, //
                0x00, 0x00, //
                0x06, 0x00, 0x00, 0x00, //
                0x48, 0x45, 0x4c, 0x4c, 0x4f, 0x00, //
                0x00, 0x00, //
                0x04, 0x00, 0x00, 0x00, //
                0x42, 0x59, 0x45, 0x00, //
                0x08, 0x00, 0x00, 0x00, //
                0x47, 0x4f, 0x4f, 0x44, 0x42, 0x59, 0x45, 0x00,
            ], RepresentationFormat::CdrLe).unwrap(),
            v
        );
    }


}
