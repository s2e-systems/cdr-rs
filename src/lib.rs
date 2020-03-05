//! A serialization/deserialization implementation for Common Data Representation.
//!
//! # Examples
//!
//! ```rust
//! use cdr::{CdrBe, Infinite};
//! use serde_derive::{Deserialize, Serialize};
//!
//! #[derive(Deserialize, Serialize, PartialEq)]
//! struct Point {
//!     x: f64,
//!     y: f64,
//! }
//!
//! #[derive(Deserialize, Serialize, PartialEq)]
//! struct Polygon(Vec<Point>);
//!
//! fn main() {
//!     let triangle = Polygon(vec![Point { x: -1.0, y: -1.0 },
//!                                 Point { x: 1.0, y: -1.0 },
//!                                 Point { x: 0.0, y: 0.73 }]);
//!
//!     let encoded = cdr::serialize::<_, _, CdrBe>(&triangle, Infinite).unwrap();
//!     let decoded = cdr::deserialize::<Polygon>(&encoded[..]).unwrap();
//!
//!     assert!(triangle == decoded);
//! }
//! ```

use std::convert::TryFrom;
pub mod de;
#[doc(inline)]
pub use crate::de::Deserializer;

mod encapsulation;
// pub use crate::encapsulation::{CdrBe, CdrLe, Encapsulation, PlCdrBe, PlCdrLe};

mod error;
pub use crate::error::{Error, Result};

pub mod ser;
#[doc(inline)]
pub use crate::ser::Serializer;

pub mod size;
#[doc(inline)]
pub use crate::size::{Bounded, Infinite, SizeLimit};

use std::io::{Read, Write};

const ENCAPSULATION_HEADER_SIZE: usize = 4;

pub enum RepresentationFormat {
    CdrBe = 0x0000,
    CdrLe = 0x0001,
    PlCdrBe = 0x0002,
    PlCdrLe = 0x0003,
}

impl RepresentationFormat {
    fn id(&self) -> u16{
        match self {
            &RepresentationFormat::CdrBe => 0x0000,
            &RepresentationFormat::CdrLe => 0x0001,
            &RepresentationFormat::PlCdrBe => 0x0002,
            &RepresentationFormat::PlCdrLe => 0x0003,
        }
    }

    fn option(&self) -> u16 {
        0x0000
    }

    fn endianness(&self) -> Endianness {
        match self {
            &RepresentationFormat::CdrBe | &RepresentationFormat::PlCdrBe => Endianness::BigEndian,
            &RepresentationFormat::CdrLe | &RepresentationFormat::PlCdrLe => Endianness::LittleEndian,
        }
    }
}

impl TryFrom<[u8;4]> for RepresentationFormat {
    type Error = Error;

    fn try_from(value: [u8;4]) -> std::result::Result<Self, Self::Error> {
        let representation_value = u16::from_be_bytes([value[0], value[1]]);
        match representation_value {
            0x0000 => Ok(RepresentationFormat::CdrBe),
            0x0001 => Ok(RepresentationFormat::CdrLe),
            0x0002 => Ok(RepresentationFormat::PlCdrBe),
            0x0003 => Ok(RepresentationFormat::PlCdrLe),
            _ => Err(Error::InvalidEncapsulation),
        }
    }
}

enum Endianness {
    BigEndian,
    LittleEndian,
}

/// Returns the size that an object would be if serialized with a encapsulation.
pub fn calc_serialized_size<T: ?Sized>(value: &T) -> usize
where
    T: serde::Serialize,
{
    size::calc_serialized_data_size(value)// + encapsulation::ENCAPSULATION_HEADER_SIZE
}

/// Given a maximum size limit, check how large an object would be if it were
/// to be serialized with a encapsulation.
pub fn calc_serialized_size_bounded<T: ?Sized>(value: &T, max: usize) -> Result<usize>
where
    T: serde::Serialize,
{
    if max < ENCAPSULATION_HEADER_SIZE {
        Err(Error::SizeLimit)
    } else {
        size::calc_serialized_data_size_bounded(value, max)
            .map(|size| size + ENCAPSULATION_HEADER_SIZE)
    }
}

/// Serializes a serializable object into a `Vec` of bytes with the encapsulation.
pub fn serialize<T: ?Sized, S>(value: &T, representation_format: RepresentationFormat, size_limit: S) -> Result<Vec<u8>>
where
    T: serde::Serialize,
    S: SizeLimit,
{
    let mut writer = match size_limit.limit() {
        Some(limit) => {
            let actual_size = calc_serialized_size_bounded(value, limit)?;
            Vec::with_capacity(actual_size as usize)
        }
        None => {
            let size = calc_serialized_size(value) as usize;
            Vec::with_capacity(size)
        }
    };

    serialize_into(&mut writer, value, representation_format, Infinite)?;
    Ok(writer)
}

/// Serializes an object directly into a `Write` with the encapsulation.
pub fn serialize_into<W, T: ?Sized, S>(writer: W, value: &T, representation_format: RepresentationFormat, size_limit: S) -> Result<()>
where
    W: Write,
    T: serde::ser::Serialize,
    S: SizeLimit,
{
    if let Some(limit) = size_limit.limit() {
        calc_serialized_size_bounded(value, limit)?;
    }

    // Header is always serialized as BigEndian
    let mut serializer = Serializer::new(writer, &RepresentationFormat::CdrBe);
    serde::Serialize::serialize(&representation_format.id(), &mut serializer)?;
    serde::Serialize::serialize(&representation_format.option(), &mut serializer)?;

    serializer.set_representation_format(&representation_format);

    serializer.reset_pos();
    serde::Serialize::serialize(value, &mut serializer)
}

/// Deserializes a slice of bytes into an object.
pub fn deserialize<'de, T>(bytes: &[u8]) -> Result<T>
where
    T: serde::Deserialize<'de>,
{
    deserialize_from::<_, _, _>(bytes, Infinite)
}

/// Deserializes an object directly from a `Read`.
pub fn deserialize_from<'de, R, T, S>(reader: R, size_limit: S) -> Result<T>
where
    R: Read,
    T: serde::Deserialize<'de>,
    S: SizeLimit,
{
    // Create a deserializer to process the header
    let mut deserializer = Deserializer::new(reader, &RepresentationFormat::CdrBe, size_limit);

    let v: [u8; ENCAPSULATION_HEADER_SIZE] =
        serde::Deserialize::deserialize(&mut deserializer)?;

    // Set the representation format based on the header
    deserializer.set_representation_format(&RepresentationFormat::try_from(v)?);

    // Deserialize the rest of the data
    deserializer.reset_pos();
    serde::Deserialize::deserialize(&mut deserializer)
    
}
