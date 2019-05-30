// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno::{PinnedBuf, Buf};
use deno::bindings::{BindingOpResult, new_binding_error};
use futures;

#[macro_use]
extern crate deno;

pub fn op_test_op(
  is_sync: bool,
  data: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> BindingOpResult {
    if !is_sync {
        return BindingOpResult::Async(Box::new(futures::future::err(new_binding_error(String::from("Async not supported!")))));
    }
    if let Some(buf) = zero_copy {
        let data_str = std::str::from_utf8(&data[..]).unwrap();
        let buf_str = std::str::from_utf8(&buf[..]).unwrap();
        println!("Hello from native bindings. data: {} | zero_copy: {}", data_str, buf_str);
    }
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    BindingOpResult::Sync(Ok(result_box))
}

declare_binding_function!(test_op, op_test_op);