#[macro_use]
extern crate deno;
extern crate futures;

use deno::CoreOp;
use deno::Op;
use deno::PluginInitContext;
use deno::{Buf, PinnedBuf};
use futures::future::FutureExt;

fn init(context: &mut dyn PluginInitContext) {
  context.register_op("test_io_sync", Box::new(op_test_io_sync));
  context.register_op("test_io_async", Box::new(op_test_io_async));
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

pub fn op_test_io_async(data: &[u8], zero_copy: Option<PinnedBuf>) -> CoreOp {
  let data_str = std::str::from_utf8(&data[..]).unwrap().to_string();
  let fut = async move {
    if let Some(buf) = zero_copy {
      let buf_str = std::str::from_utf8(&buf[..]).unwrap();
      println!(
        "Hello from native bindings. data: {} | zero_copy: {}",
        data_str, buf_str
      );
    }
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_secs(1));
      tx.send(Ok(())).unwrap();
    });
    assert!(rx.await.is_ok());
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    Ok(result_box)
  };

  Op::Async(fut.boxed())
}

#[test]
fn placeholder() {
  // This test is just here to make sure the plugin gets built
}
