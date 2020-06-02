use deno_core::plugin_api::Buf;
use deno_core::plugin_api::Interface;
use deno_core::plugin_api::Op;
use deno_core::plugin_api::ZeroCopyBuf;
use futures::future::FutureExt;

#[no_mangle]
pub fn deno_plugin_init(interface: &mut dyn Interface) {
  interface.register_op("testSync", op_test_sync);
  interface.register_op("testAsync", op_test_async);
  interface.register_op("testResources", op_test_resources);
}

fn op_test_sync(
  _interface: &mut dyn Interface,
  data: &[u8],
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  let data_str = std::str::from_utf8(&data[..]).unwrap();
  let zero_copy = zero_copy.to_vec();
  if !zero_copy.is_empty() {
    println!("Hello from plugin. data: {}", data_str);
  }
  for (idx, buf) in zero_copy.iter().enumerate() {
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!("zero_copy[{}]: {}", idx, buf_str);
  }
  let result = b"test";
  let result_box: Buf = Box::new(*result);
  Op::Sync(result_box)
}

fn op_test_async(
  _interface: &mut dyn Interface,
  data: &[u8],
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  let zero_copy = zero_copy.to_vec();
  if !zero_copy.is_empty() {
    let data_str = std::str::from_utf8(&data[..]).unwrap().to_string();
    println!("Hello from plugin. data: {}", data_str);
  }
  let fut = async move {
    for (idx, buf) in zero_copy.iter().enumerate() {
      let buf_str = std::str::from_utf8(&buf[..]).unwrap();
      println!("zero_copy[{}]: {}", idx, buf_str);
    }
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_secs(1));
      tx.send(Ok(())).unwrap();
    });
    assert!(rx.await.is_ok());
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    result_box
  };

  Op::Async(fut.boxed())
}

struct TestResource {
  noise: String,
}

fn op_test_resources(
  interface: &mut dyn Interface,
  _data: &[u8],
  _zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  let rid = {
    // `add()`
    let rc = Box::new(TestResource {
      noise: "woof".to_owned(),
    });
    let rt = interface.resource_table();
    let mut rt = rt.borrow_mut();
    rt.add("test_resource", rc)
  };
  {
    // `has()`
    let found = interface.resource_table().borrow().has(rid);
    assert!(found);
  }
  {
    // `get()`
    let rt = interface.resource_table();
    let rt = rt.borrow();
    let rc = rt.get::<TestResource>(rid).unwrap();
    assert_eq!(&rc.noise, "woof");
  }
  {
    // `get_mut()`
    let rt = interface.resource_table();
    let mut rt = rt.borrow_mut();
    let rc = rt.get_mut::<TestResource>(rid).unwrap();
    assert_eq!(&rc.noise, "woof");
    rc.noise = "mooh".to_owned();
  }
  {
    // The resource's internal state should have changed.
    let rt = interface.resource_table();
    let rt = rt.borrow();
    let rc = rt.get::<TestResource>(rid).unwrap();
    assert_eq!(&rc.noise, "mooh");
  }
  {
    // `close()`
    let rt = interface.resource_table();
    let mut rt = rt.borrow_mut();
    let found = rt.close(rid).is_some();
    assert!(found);
  }
  {
    let rt = interface.resource_table();
    let mut rt = rt.borrow_mut();
    // After `close()` the resource should be gone.
    let found1 = rt.has(rid);
    assert!(!found1);
    let found2 = rt.close(rid).is_some();
    assert!(!found2);
  }
  {
    // add and leave in table
    let rc = Box::new(TestResource {
      noise: "woof".to_owned(),
    });
    let rt = interface.resource_table();
    let mut rt = rt.borrow_mut();
    rt.add("test_resource", rc);
  }
  Op::Sync(Default::default())
}
