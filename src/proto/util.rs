use std::io::{self, Read};
use std::iter::Iterator;

use super::Error;
use crate::device::Handle;

pub trait HandleExt {
    fn with_post_read_op<F, T>(&self, f: F) -> Result<T, Error>
    where
        F: FnMut(&Self) -> Result<T, Error>;

    fn with_post_write_op<F, T>(&self, f: F) -> Result<T, Error>
    where
        F: FnMut(&Self) -> Result<T, Error>;
}

impl HandleExt for Handle {
    fn with_post_read_op<F, T>(&self, mut f: F) -> Result<T, Error>
    where
        F: FnMut(&Self) -> Result<T, Error>,
    {
        let ret = f(self)?;
        tracing::trace!("read bulk with empty slice");
        self.read(&mut [])?;
        Ok(ret)
    }

    fn with_post_write_op<F, T>(&self, mut f: F) -> Result<T, Error>
    where
        F: FnMut(&Self) -> Result<T, Error>,
    {
        let ret = f(self)?;
        tracing::trace!("write bulk with empty slice");
        self.write(&[])?;
        Ok(ret)
    }
}

pub fn fill_buf<R: Read>(r: &mut R, mut buf: &mut [u8]) -> io::Result<usize> {
    let mut count = 0;
    while !buf.is_empty() {
        match r.read(buf)? {
            0 => break,
            n => {
                buf = &mut buf[n..];
                count += n;
            }
        }
    }
    Ok(count)
}

pub struct BatchIterator {
    bytes_left: u64,
    chunk_size: u32,
    batch_size: u32,
}

impl BatchIterator {
    pub fn new(bytes: u64, chunk_size: u32, chunks_per_batch: u32) -> Self {
        Self {
            bytes_left: bytes,
            chunk_size,
            batch_size: chunk_size * chunks_per_batch,
        }
    }
}

impl Iterator for BatchIterator {
    type Item = Batch;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes_left > 0 {
            let (size, last) = if self.bytes_left <= self.batch_size as u64 {
                (self.bytes_left as u32, true)
            } else {
                (self.batch_size, false)
            };

            let chunks = if size % self.chunk_size == 0 {
                size / self.chunk_size
            } else {
                size / self.chunk_size + 1
            };

            self.bytes_left -= size as u64;

            Some(Batch {
                effective_size: size,
                last,
                chunk_size: self.chunk_size,
                chunks,
            })
        } else {
            None
        }
    }
}

pub struct Batch {
    chunks: u32,
    chunk_size: u32,
    effective_size: u32,
    last: bool,
}

impl Batch {
    #[inline]
    pub fn size(&self) -> u32 {
        self.chunks * self.chunk_size
    }

    #[inline]
    pub fn effective_size(&self) -> u32 {
        self.effective_size
    }

    #[inline]
    pub fn is_last(&self) -> bool {
        self.last
    }

    #[inline]
    pub fn chunks(&self) -> impl Iterator<Item = u32> {
        0..self.chunks
    }
}
