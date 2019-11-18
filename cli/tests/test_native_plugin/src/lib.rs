use deno::CoreOp;
use deno::Op;
use deno::PluginInitContext;
use deno::{Buf, PinnedBuf};

#[macro_use]
extern crate deno;

fn init(context: &mut dyn PluginInitContext) {
  context.register_op("test_io_sync", Box::new(op_test_io_sync));
}
init_fn!(init);

pub fn op_test_io_sync(data: &[u8], zero_copy: Option<PinnedBuf>) -> CoreOp {
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
  Op::Sync(result_box)
}
