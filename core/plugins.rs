use crate::isolate::Op;
use crate::libdeno::PinnedBuf;
use std::fmt;
use std::io;

pub type PluginResult<T> = std::result::Result<T, PluginError>;

#[derive(Debug)]
pub struct PluginError {
  repr: Repr,
}

#[derive(Debug)]
enum Repr {
  Simple(String),
  IoErr(io::Error),
}

pub fn new_plugin_error(msg: String) -> PluginError {
  PluginError {
    repr: Repr::Simple(msg),
  }
}

impl fmt::Display for PluginError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.repr {
      Repr::Simple(ref err_str) => f.pad(err_str),
      Repr::IoErr(ref err) => err.fmt(f),
    }
  }
}

impl std::error::Error for PluginError {
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

impl From<io::Error> for PluginError {
  #[inline]
  fn from(err: io::Error) -> Self {
    Self {
      repr: Repr::IoErr(err),
    }
  }
}

/// Base result type for a plugin op represents either a Sync or Async value
pub type PluginOp = Op<PluginError>;

/// Complete return type for a plugin op including Sync errors
pub type PluginOpResult = PluginResult<PluginOp>;

/// Funciton type for plugin ops
pub type PluginOpDispatchFn =
  fn(is_sync: bool, data: &[u8], zero_copy: Option<PinnedBuf>)
    -> PluginOpResult;

#[macro_export]
macro_rules! declare_plugin_op {
  ($name:ident, $fn:path) => {
    #[no_mangle]
    pub fn $name(
      is_sync: bool,
      data: &[u8],
      zero_copy: Option<PinnedBuf>,
    ) -> PluginOpResult {
      $fn(is_sync, data, zero_copy)
    }
  };
}
