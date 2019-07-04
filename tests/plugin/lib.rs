// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![feature(async_await, await_macro, futures_api)]
use deno::CoreOp;
use deno::Op;
use deno::{Buf, PinnedBuf};
use deno::plugins::PluginOp;
use futures::future::Future;
use futures::future::FutureExt;
use futures::channel::oneshot::channel;

#[macro_use]
extern crate deno;

pub fn op_test_op(data: &[u8], zero_copy: Option<PinnedBuf>) -> PluginOp {
  if let Some(buf) = zero_copy {
    let data_str = std::str::from_utf8(&data[..]).unwrap();
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from native bindings. data: {} | zero_copy: {}",
      data_str, buf_str
    );
  }
  let result = b"test";
  let result_box: Buf = Box::new(*result);
  PluginOp::Sync(result_box)
}

declare_plugin_op!(test_op, op_test_op);

pub fn op_async_test_op(data: &[u8], zero_copy: Option<PinnedBuf>) -> PluginOp {
  if let Some(buf) = zero_copy {
    let data_str = std::str::from_utf8(&data[..]).unwrap();
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from native bindings. data: {} | zero_copy: {}",
      data_str, buf_str
    );
  }
  let (sender, receiver) = channel::<u32>();
  std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_millis(2000));
    assert!(sender.send(5).is_ok());
  });
  let op = async {
    receiver.await.map_err(|_| panic!("Channel error!")).unwrap();
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    result_box
  }.boxed();
  PluginOp::Async(op)
}

declare_plugin_op!(async_test_op, op_async_test_op);
