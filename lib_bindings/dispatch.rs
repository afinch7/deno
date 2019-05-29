// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
pub use deno::{Buf, PinnedBuf};
pub use hyper::rt::Future;
use crate::errors::{BindingError, BindingResult};

/// Dispatch context to give ops access to various context functions
pub trait BindingDispatchContext {
  // TODO(afinch7) add dispatch context functions
}

pub type OpAsyncFuture = Box<dyn Future<Item = Buf, Error = BindingError> + Send>;

pub enum OpResult {
  Sync(BindingResult<Buf>),
  Async(OpAsyncFuture),
}

/// Dispatch funciton type
/// base is a placeholder value for now not sure what we want to use there
pub type OpDispatchFn =
  fn(is_sync: bool, data: &[u8], zeroCopy: Option<PinnedBuf>)
    -> OpResult;

#[macro_export]
macro_rules! declare_binding_function {
  ($name:ident, $fn:path) => {
    #[no_mangle]
    pub extern "C" fn $name(is_sync: bool, data: &[u8], zero_copy: Option<PinnedBuf>) -> OpResult {
      // make sure the constructor is the correct type.
      let dispatch_fn: $crate::dispatch::OpDispatchFn = $fn;
      dispatch_fn(is_sync, data, zero_copy)
    }
  };
}