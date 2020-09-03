// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, Value};
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ErrBox;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use std::rc::Rc;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  let t = (); // Temp.

  i.register_op("op_resources", s.stateful_json_op_sync(t, op_resources));
  i.register_op("op_close", s.stateful_json_op_sync(t, op_close));
}

fn op_resources(state: &State, _: (), _args: Value, _zero_copy: &mut [ZeroCopyBuf]) -> Result<Value, ErrBox> {
  let resource_table = state.resource_table.borrow();
  let serialized_resources = resource_table.entries();
  Ok(json!(serialized_resources))
}

/// op_close removes a resource from the resource table.
fn op_close(state: &State, _: (), args: Value, _zero_copy: &mut [ZeroCopyBuf]) -> Result<Value, ErrBox> {
  #[derive(Deserialize)]
  struct CloseArgs {
    rid: i32,
  }
  let args: CloseArgs = serde_json::from_value(args)?;
  state.resource_table.borrow_mut().close(args.rid as u32).ok_or_else(ErrBox::bad_resource_id)?;
  Ok(json!({}))
}
