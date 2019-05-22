// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std::io;
use std::fmt;

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

pub fn new(msg: String) -> BindingError {
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
