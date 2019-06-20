// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno::{PinnedBuf, Buf};
use deno::plugins::{PluginOpResult, new_plugin_error};
use deno::Op;

#[macro_use]
extern crate deno;

pub fn op_test_op(
  is_sync: bool,
  data: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> PluginOpResult {
    if !is_sync {
        return Err(new_plugin_error(String::from("Async not supported!")));
    }
    if let Some(buf) = zero_copy {
        let data_str = std::str::from_utf8(&data[..]).unwrap();
        let buf_str = std::str::from_utf8(&buf[..]).unwrap();
        println!("Hello from native bindings. data: {} | zero_copy: {}", data_str, buf_str);
    }
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    Ok(Op::Sync(result_box))
}

declare_plugin_op!(test_op, op_test_op);