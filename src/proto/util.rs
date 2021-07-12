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
