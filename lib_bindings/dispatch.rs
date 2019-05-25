// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
pub use deno::{Buf, PinnedBuf};
pub use hyper::rt::Future;
use crate::errors::BindingError;

/// Dispatch context to give ops access to various context functions
pub trait BindingDispatchContext {
  // TODO(afinch7) add dispatch context functions
}

/// Result future of a op completion
pub type OpWithError = dyn Future<Item = Buf, Error = BindingError> + Send;

/// Dispatch funciton type
/// base is a placeholder value for now not sure what we want to use there
pub type OpDispatchFn =
  fn(state: &BindingDispatchContext, data: Option<PinnedBuf>)
    -> Box<OpWithError>;