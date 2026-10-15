//! Bounds-checked little-endian reads over a byte slice.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("read out of bounds for {what} at {offset:#x} (+{len:#x}) in a {size:#x}-byte buffer")]
    OutOfBounds { what: &'static str, offset: usize, len: usize, size: usize },
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;

/// A borrowed byte range with checked accessors. All integers are little-endian.
#[derive(Clone, Copy)]
pub struct Buf<'a>(pub &'a [u8]);

impl<'a> Buf<'a> {
    pub fn len(&self) -> usize { self.0.len() }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
    pub fn bytes(&self) -> &'a [u8] { self.0 }

    pub fn check(&self, offset: usize, len: usize, what: &'static str) -> Result<()> {
        if offset > self.0.len() || len > self.0.len() - offset {
            return Err(FormatError::OutOfBounds { what, offset, len, size: self.0.len() });
        }
        Ok(())
    }
    pub fn sub(&self, offset: usize, len: usize, what: &'static str) -> Result<Buf<'a>> {
        self.check(offset, len, what)?;
        Ok(Buf(&self.0[offset..offset + len]))
    }
    pub fn tail(&self, offset: usize, what: &'static str) -> Result<Buf<'a>> {
        self.check(offset, 0, what)?;
        Ok(Buf(&self.0[offset..]))
    }
    pub fn u8(&self, o: usize) -> Result<u8> { self.check(o, 1, "u8")?; Ok(self.0[o]) }
    pub fn i8(&self, o: usize) -> Result<i8> { Ok(self.u8(o)? as i8) }
    pub fn u16(&self, o: usize) -> Result<u16> { self.check(o, 2, "u16")?; Ok(u16::from_le_bytes([self.0[o], self.0[o + 1]])) }
    pub fn i16(&self, o: usize) -> Result<i16> { Ok(self.u16(o)? as i16) }
    pub fn u32(&self, o: usize) -> Result<u32> { self.check(o, 4, "u32")?; Ok(u32::from_le_bytes(self.0[o..o + 4].try_into().unwrap())) }
    pub fn i32(&self, o: usize) -> Result<i32> { Ok(self.u32(o)? as i32) }
    pub fn f32(&self, o: usize) -> Result<f32> { Ok(f32::from_bits(self.u32(o)?)) }

    /// Reads `count` plain-old-data records of type `T` starting at `offset`.
    pub fn pod_slice<T: bytemuck::Pod>(&self, offset: usize, count: usize, what: &'static str) -> Result<Vec<T>> {
        let len = count * std::mem::size_of::<T>();
        self.check(offset, len, what)?;
        Ok(bytemuck::pod_collect_to_vec(&self.0[offset..offset + len]))
    }
    pub fn pod<T: bytemuck::Pod>(&self, offset: usize, what: &'static str) -> Result<T> {
        let len = std::mem::size_of::<T>();
        self.check(offset, len, what)?;
        Ok(bytemuck::pod_read_unaligned(&self.0[offset..offset + len]))
    }
}

pub fn invalid<T>(msg: impl Into<String>) -> Result<T> { Err(FormatError::Invalid(msg.into())) }
