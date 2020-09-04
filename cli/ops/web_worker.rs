// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::ops::json_op;
use crate::state::State;
use crate::web_worker::WebWorkerHandle;
use crate::worker::WorkerEvent;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpManager;
use futures::channel::mpsc;
use std::convert::From;
use std::rc::Rc;

pub fn web_worker_op<D>(
  sender: mpsc::Sender<WorkerEvent>,
  dispatcher: D,
) -> impl Fn(Rc<State>, Value, BufVec) -> Result<JsonOp, ErrBox>
where
  D: Fn(&mpsc::Sender<WorkerEvent>, Value, BufVec) -> Result<JsonOp, ErrBox>,
{
  move |_state: Rc<State>,
        args: Value,
        zero_copy: BufVec|
        -> Result<JsonOp, ErrBox> { dispatcher(&sender, args, zero_copy) }
}

pub fn web_worker_op2<D>(
  handle: WebWorkerHandle,
  sender: mpsc::Sender<WorkerEvent>,
  dispatcher: D,
) -> impl Fn(Rc<State>, Value, BufVec) -> Result<JsonOp, ErrBox>
where
  D: Fn(
    WebWorkerHandle,
    &mpsc::Sender<WorkerEvent>,
    Value,
    BufVec,
  ) -> Result<JsonOp, ErrBox>,
{
  move |_state: Rc<State>,
        args: Value,
        zero_copy: BufVec|
        -> Result<JsonOp, ErrBox> {
    dispatcher(handle.clone(), &sender, args, zero_copy)
  }
}

pub fn init(
  s: &Rc<State>,
  sender: &mpsc::Sender<WorkerEvent>,
  handle: WebWorkerHandle,
) {
  s.register_op(
    "op_worker_post_message",
    s.core_op(json_op(web_worker_op(
      sender.clone(),
      op_worker_post_message,
    ))),
  );
  s.register_op(
    "op_worker_close",
    s.core_op(json_op(web_worker_op2(
      handle,
      sender.clone(),
      op_worker_close,
    ))),
  );
}

/// Post message to host as guest worker
fn op_worker_post_message(
  sender: &mpsc::Sender<WorkerEvent>,
  _args: Value,
  data: BufVec,
) -> Result<JsonOp, ErrBox> {
  assert_eq!(data.len(), 1, "Invalid number of arguments");
  let d = Vec::from(&*data[0]).into_boxed_slice();
  let mut sender = sender.clone();
  sender
    .try_send(WorkerEvent::Message(d))
    .expect("Failed to post message to host");
  Ok(JsonOp::Sync(json!({})))
}

/// Notify host that guest worker closes
fn op_worker_close(
  handle: WebWorkerHandle,
  sender: &mpsc::Sender<WorkerEvent>,
  _args: Value,
  _data: BufVec,
) -> Result<JsonOp, ErrBox> {
  let mut sender = sender.clone();
  // Notify parent that we're finished
  sender.close_channel();
  // Terminate execution of current worker
  handle.terminate();
  Ok(JsonOp::Sync(json!({})))
}
