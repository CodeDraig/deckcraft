//! Random-access input byte sources.

use std::io;

/// Random-access, read-only byte source (same shape as the other FilmCraft demuxers').
pub trait ByteSource {
    fn len(&self) -> u64;
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ByteSource for [u8] {
    fn len(&self) -> u64 {
        <[u8]>::len(self) as u64
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let eof = || io::Error::new(io::ErrorKind::UnexpectedEof, "read past end");
        let a = usize::try_from(offset).map_err(|_| eof())?;
        buf.copy_from_slice(self.get(a..a.checked_add(buf.len()).ok_or_else(eof)?).ok_or_else(eof)?);
        Ok(())
    }
}

impl ByteSource for Vec<u8> {
    fn len(&self) -> u64 {
        self.as_slice().len() as u64
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.as_slice().read_at(offset, buf)
    }
}

impl<T: ByteSource + ?Sized> ByteSource for &T {
    fn len(&self) -> u64 {
        (**self).len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_at(offset, buf)
    }
}
