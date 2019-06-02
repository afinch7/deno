use crate::isolate::Buf;
use crate::libdeno::PinnedBuf;
use futures::Future;
use std::fmt;
use std::io;

pub type BindingResult<T> = std::result::Result<T, BindingError>;

#[derive(Debug)]
pub struct BindingError {
  repr: Repr,
}

#[derive(Debug)]
enum Repr {
  Simple(String),
  IoErr(io::Error),
}

pub fn new_binding_error(msg: String) -> BindingError {
  BindingError {
    repr: Repr::Simple(msg),
  }
}

impl fmt::Display for BindingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.repr {
      Repr::Simple(ref err_str) => f.pad(err_str),
      Repr::IoErr(ref err) => err.fmt(f),
    }
  }
}

impl std::error::Error for BindingError {
  fn description(&self) -> &str {
    match self.repr {
      Repr::Simple(ref msg) => msg.as_str(),
      Repr::IoErr(ref err) => err.description(),
    }
  }

  fn cause(&self) -> Option<&dyn std::error::Error> {
    match self.repr {
      Repr::Simple(ref _msg) => None,
      Repr::IoErr(ref err) => Some(err),
    }
  }
}

impl From<io::Error> for BindingError {
  #[inline]
  fn from(err: io::Error) -> Self {
    Self {
      repr: Repr::IoErr(err),
    }
  }
}

pub type BindingOpAsyncFuture =
  Box<dyn Future<Item = Buf, Error = BindingError> + Send>;

pub enum BindingOpSyncOrAsync {
  Sync(Buf),
  Async(BindingOpAsyncFuture),
}

pub type BindingOpResult = BindingResult<BindingOpSyncOrAsync>;

/// Dispatch funciton type
/// base is a placeholder value for now not sure what we want to use there
pub type BindingOpDispatchFn =
  fn(is_sync: bool, data: &[u8], zeroCopy: Option<PinnedBuf>)
    -> BindingOpResult;

#[macro_export]
macro_rules! declare_binding_function {
  ($name:ident, $fn:path) => {
    #[no_mangle]
    pub fn $name(
      is_sync: bool,
      data: &[u8],
      zero_copy: Option<PinnedBuf>,
    ) -> BindingOpResult {
      $fn(is_sync, data, zero_copy)
    }
  };
}
