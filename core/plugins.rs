// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate::Buf;
use crate::isolate::CoreOp;
use crate::isolate::Op;
use crate::libdeno::PinnedBuf;
use futures::future::{FutureExt, TryFutureExt};
use futures::future::Future;
use std::pin::Pin;

pub type PluginOpAsyncFuture = Pin<Box<dyn Future<Output = Buf> + Send>>;

pub enum PluginOp {
  Sync(Buf),
  Async(PluginOpAsyncFuture),
}

impl Into<CoreOp> for PluginOp {
  fn into(self) -> CoreOp {
    match self {
      PluginOp::Sync(buf) => Op::Sync(buf),
      PluginOp::Async(fut) => Op::Async(Box::new(fut.unit_error().boxed().compat())),
    }
  }
}

/// Funciton type for plugin ops
pub type PluginDispatchFn =
  fn(data: &[u8], zero_copy: Option<PinnedBuf>) -> PluginOp;

#[macro_export]
macro_rules! declare_plugin_op {
  ($name:ident, $fn:path) => {
    #[no_mangle]
    pub fn $name(data: &[u8], zero_copy: Option<PinnedBuf>) -> PluginOp {
      $fn(data, zero_copy)
    }
  };
}
