// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::errors::DenoResult;
use crate::state::ThreadSafeState;
use deno::CustomOpId;
use deno::OpId;
use deno_lib_bindings::dispatch::{BindingDispatchContext, OpDispatchFn};
use deno_lib_bindings::errors::BindingResult;
use deno_lib_bindings::plugin::{BindingInitContext, BindingPlugin};
use libloading::{Library, Symbol};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

pub struct DenoDispatchContext;

impl DenoDispatchContext {
  pub fn new() -> Self {
    Self {}
  }
}

impl BindingDispatchContext for DenoDispatchContext {
}

pub struct DenoInitContext {
  state: ThreadSafeState,
  op_namespace: String,
  /// List of new op ids to register with the v8 isolate
  pub custom_op_ids: Mutex<Vec<CustomOpId>>,
}

impl DenoInitContext {
  pub fn new(state: ThreadSafeState, op_namespace: String) -> Self {
    Self {
      state,
      op_namespace,
      custom_op_ids: Mutex::new(Vec::new()),
    }
  }
}

impl BindingInitContext for DenoInitContext {
  fn register_op(
    &self,
    name: String,
    dispatch: OpDispatchFn,
  ) -> BindingResult<()> {
    let next_op_id: OpId =
      self.state.binding_next_op_id.fetch_add(1, Ordering::SeqCst);
    let mut binding_id_map = self.state.binding_op_id_map.write().unwrap();
    binding_id_map.insert(next_op_id.clone(), dispatch);
    let mut new_op_id_list = self.custom_op_ids.lock().unwrap();
    new_op_id_list.push((self.op_namespace.clone(), name, next_op_id));
    Ok(())
  }
}

pub type BindingLoadResult = Box<BindingPlugin>;

lazy_static! {
  static ref LIBRARY_LIST: Mutex<Vec<Library>> = Mutex::new(Vec::new());
}

// Plugin system based off of https://michael-f-bryan.github.io/rust-ffi-guide/dynamic_loading.html

pub unsafe fn load_binding_plugin<P: Into<PathBuf>>(
  lib_path: P,
) -> DenoResult<BindingLoadResult> {
  type PluginCreate = unsafe fn() -> *mut BindingPlugin;

  let lib_path: PathBuf = lib_path.into();

  debug!("LOADING NATIVE BINDING LIB: {:#?}", lib_path);

  let lib = Library::new(lib_path).unwrap();

  // We place the loaded lib into a vec so that it's contents
  // remain statically located in memory.
  let mut lib_list = LIBRARY_LIST.lock().unwrap();
  lib_list.push(lib);

  let lib = lib_list.last().unwrap();

  let constructor: Symbol<PluginCreate> =
    lib.get(b"_binding_plugin_create").unwrap();
  let boxed_raw = constructor();
  let plugin = Box::from_raw(boxed_raw);

  println!("Loaded plugin: {}", plugin.name());

  return Ok(plugin);
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno::OpId;
  use deno_lib_bindings::dispatch::{
    BindingDispatchContext, Future, OpDispatchFn,
  };
  use deno_lib_bindings::errors::BindingResult;
  use deno_lib_bindings::plugin::BindingInitContext;
  use std::collections::HashMap;
  use std::env;
  use std::path::Path;
  use std::sync::atomic::AtomicU32;
  use std::sync::atomic::Ordering;
  use std::sync::Mutex;

  lazy_static! {
    static ref NEXT_OP_ID: AtomicU32 = AtomicU32::new(0);
    static ref OP_ID_TABLE: Mutex<HashMap<OpId, OpDispatchFn>> =
      Mutex::new(HashMap::new());
    static ref V8_OP_ID_MOCK_TABLE: Mutex<HashMap<String, OpId>> =
      Mutex::new(HashMap::new());
  }

  fn new_op_id() -> OpId {
    let next_op_id = NEXT_OP_ID.fetch_add(1, Ordering::SeqCst);
    next_op_id as OpId
  }

  fn set_op_id_in_v8_mock(name: String, op_id: OpId) {
    let mut op_id_table = V8_OP_ID_MOCK_TABLE.lock().unwrap();
    op_id_table.insert(name, op_id);
  }

  fn get_op_id_from_v8_mock(name: &str) -> Option<OpId> {
    let op_id_table = V8_OP_ID_MOCK_TABLE.lock().unwrap();
    match op_id_table.get(name) {
      Some(v) => Some(v.clone()),
      None => None,
    }
  }

  pub struct MockInitContext {
    plugin_name: String,
  }

  impl MockInitContext {
    pub fn new(plugin_name: String) -> Self {
      MockInitContext { plugin_name }
    }
  }

  impl BindingInitContext for MockInitContext {
    fn register_op(
      &self,
      name: String,
      dispatch: OpDispatchFn,
    ) -> BindingResult<()> {
      let op_id = new_op_id();
      let mut op_id_table = OP_ID_TABLE.lock().unwrap();
      op_id_table.insert(op_id, dispatch);
      set_op_id_in_v8_mock(format!("{}.{}", self.plugin_name, name), op_id);
      Ok(())
    }
  }

  pub struct MockDispatchContext;

  impl MockDispatchContext {
    pub fn new() -> Self {
      MockDispatchContext {}
    }
  }

  impl BindingDispatchContext for MockDispatchContext {}

  #[test]
  fn test_loader() {
    println!("CWD {:#?}", env::current_dir().unwrap());
    println!("PLUGIN PATH {:#?}", concat!(env!("GN_OUT_DIR"), "/rust_crates/libtest_binding_plugin.so"));
    let plugin_path = Path::new(concat!(env!("GN_OUT_DIR"), "/rust_crates/libtest_binding_plugin.so"))
      .canonicalize()
      .unwrap();
    println!("PLUGIN PATH {:#?}", plugin_path);
    let plugin = unsafe { load_binding_plugin(plugin_path).unwrap() };

    let dispatch_ctx = MockDispatchContext::new();
    let next_op_id = NEXT_OP_ID.load(Ordering::SeqCst);
    let plugin_name = plugin.name().to_string();

    let init_ctx = MockInitContext::new(plugin_name.clone());
    plugin.init(&init_ctx).unwrap();

    let v8_op_id =
      get_op_id_from_v8_mock(&format!("{}.{}", plugin_name, "testOp"));
    assert!(v8_op_id.is_some());
    let v8_op_id_num = v8_op_id.unwrap();
    assert_eq!(v8_op_id_num, next_op_id);

    let op_id_table = OP_ID_TABLE.lock().unwrap();
    let op_dispatch = op_id_table.get(&next_op_id);
    assert!(op_dispatch.is_some());
    let result_future =
      op_dispatch.unwrap()(&dispatch_ctx, None);
    let result = result_future.wait();
    assert!(result.is_ok());
    let result_buf = result.unwrap();
    assert_eq!(*result_buf, *b"test");
  }
}
