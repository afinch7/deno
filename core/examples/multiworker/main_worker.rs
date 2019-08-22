use crate::json_ops::wrap_json_op;
use crate::json_ops::EmptyResponse;
use crate::json_ops::JsonOp;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::CoreOp;
use deno::Named;
use deno::OpDispatcher;
use deno::PinnedBuf;
use serde::Deserialize;
use std::sync::Arc;

struct OpListen {
  state: ThreadSafeState,
}

impl OpListen {
  pub fn new(state: ThreadSafeState) -> Self {
    Self { state }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpListenOptions {
  address: String,
  worker_script: String,
  worker_count: u32,
}

impl OpDispatcher for OpListen {
  fn dispatch(&self, args: &[u8], buf: Option<PinnedBuf>) -> CoreOp {
    wrap_json_op(
      |args, _buf| {
        let options: OpListenOptions = serde_json::from_value(args).unwrap();

        self.state.listen(
          options.address,
          &options.worker_script,
          options.worker_count,
        );

        JsonOp::Sync(EmptyResponse)
      },
      args,
      buf,
    )
  }
}

impl Named for OpListen {
  const NAME: &'static str = "listen";
}

static MAIN_WORKER_NAMESPACE: &'static str = "mainWorker";

pub fn register_op_dispatchers(worker: Arc<Worker>) {
  let state = worker.state.clone();

  worker.register_op(MAIN_WORKER_NAMESPACE, OpListen::new(state.clone()));
}
