// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use core::ops::Deref;
use crate::isolate_state::*;
use crate::msg;
use crate::ops;
use crate::resources;
use crate::resources::ResourceId;
use crate::startup_data;
use crate::workers;
use crate::workers::WorkerBehavior;
use crate::workers::WorkerInit;
use deno_core::deno_buf;
use deno_core::Behavior;
use deno_core::Buf;
use deno_core::JSError;
use deno_core::Op;
use deno_core::StartupData;
use futures::future::*;
use futures::sync::oneshot;
use futures::Future;
use serde_json;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

pub type CompilerResult = Result<ModuleMetaData, JSError>;
type CompilerInnerResult = Result<ModuleMetaData, Option<JSError>>;
type WorkerErrReceiver = oneshot::Receiver<CompilerInnerResult>;

#[derive(Clone)]
struct CompilerShared {
  pub rid: ResourceId,
  pub worker_err_receiver: Shared<WorkerErrReceiver>,
}

lazy_static! {
  static ref C_SHARED: Mutex<Option<CompilerShared>> = Mutex::new(None);
}

pub struct CompilerBehavior {
  pub state: Arc<IsolateState>,
}

impl CompilerBehavior {
  pub fn new(state: Arc<IsolateState>) -> Self {
    Self { state }
  }
}

impl IsolateStateContainer for CompilerBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl IsolateStateContainer for &CompilerBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl Behavior for CompilerBehavior {
  fn startup_data(&mut self) -> Option<StartupData> {
    Some(startup_data::compiler_isolate_init())
  }

  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch_all(self, control, zero_copy, ops::op_selector_compiler)
  }
}

impl WorkerBehavior for CompilerBehavior {
  fn set_internal_channels(&mut self, worker_channels: WorkerChannels) {
    self.state = Arc::new(IsolateState::new(
      self.state.flags.clone(),
      self.state.argv.clone(),
      Some(worker_channels),
    ));
  }
}

// This corresponds to JS ModuleMetaData.
// TODO Rename one or the other so they correspond.
#[derive(Debug, Clone)]
pub struct ModuleMetaData {
  pub module_name: String,
  pub filename: String,
  pub media_type: msg::MediaType,
  pub source_code: Vec<u8>,
  pub maybe_output_code_filename: Option<String>,
  pub maybe_output_code: Option<Vec<u8>>,
  pub maybe_source_map_filename: Option<String>,
  pub maybe_source_map: Option<Vec<u8>>,
}

impl ModuleMetaData {
  pub fn js_source(&self) -> String {
    if self.media_type == msg::MediaType::Json {
      return format!(
        "export default {};",
        str::from_utf8(&self.source_code).unwrap()
      );
    }
    match self.maybe_output_code {
      None => str::from_utf8(&self.source_code).unwrap().to_string(),
      Some(ref output_code) => str::from_utf8(output_code).unwrap().to_string(),
    }
  }
}

fn lazy_start(parent_state: Arc<IsolateState>) -> CompilerShared {
  let mut cell = C_SHARED.lock().unwrap();
  cell
    .get_or_insert_with(|| {
      let worker_result = workers::spawn(
        CompilerBehavior::new(Arc::new(IsolateState::new(
          parent_state.flags.clone(),
          parent_state.argv.clone(),
          None,
        ))),
        WorkerInit::Script("compilerMain()".to_string()),
      );
      match worker_result {
        Ok(worker) => {
          let rid = worker.resource.rid.clone();
          // create oneshot channels and use the sender to pass back
          // results from worker future
          let (err_sender, err_receiver) =
            oneshot::channel::<CompilerInnerResult>();
          tokio::spawn(lazy(move || {
            worker.then(|result| -> Result<(), ()> {
              match result {
                Err(err) => err_sender.send(Err(Some(err))).unwrap(),
                _ => err_sender.send(Err(None)).unwrap(),
              };
              Ok(())
            })
          }));
          CompilerShared {
            rid,
            worker_err_receiver: err_receiver.shared(),
          }
        }
        Err(err) => {
          println!("{}", err.to_string());
          std::process::exit(1);
        }
      }
    }).clone()
}

fn req(specifier: &str, referrer: &str) -> Buf {
  json!({
    "specifier": specifier,
    "referrer": referrer,
  }).to_string()
  .into_boxed_str()
  .into_boxed_bytes()
}

pub fn compile_sync(
  parent_state: Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
  module_meta_data: &ModuleMetaData,
) -> Result<ModuleMetaData, JSError> {
  let req_msg = req(specifier, referrer);

  let shared = lazy_start(parent_state);

  let (local_sender, local_receiver) =
    oneshot::channel::<Result<ModuleMetaData, Option<JSError>>>();

  let compiler_rid = shared.rid.clone();
  let module_meta_data_ = module_meta_data.clone();

  tokio::spawn(lazy(move || {
    debug!("Running rust part of compile_sync");
    let send_future = resources::post_message_to_worker(compiler_rid, req_msg);
    match send_future.wait() {
      Ok(_) => {} // Do nothing if result is ok else send back None error
      _ => return Ok(local_sender.send(Err(None)).unwrap()),
    };

    let recv_future = resources::get_message_from_worker(compiler_rid);
    let res_msg = match recv_future.wait() {
      Ok(Some(v)) => v,
      _ => return Ok(local_sender.send(Err(None)).unwrap()),
    };

    let res_json = std::str::from_utf8(&res_msg).unwrap();
    Ok(
      local_sender
        .send(Ok(
          match serde_json::from_str::<serde_json::Value>(res_json) {
            Ok(serde_json::Value::Object(map)) => ModuleMetaData {
              module_name: module_meta_data_.module_name.clone(),
              filename: module_meta_data_.filename.clone(),
              media_type: module_meta_data_.media_type,
              source_code: module_meta_data_.source_code.clone(),
              maybe_output_code: match map["outputCode"].as_str() {
                Some(str) => Some(str.as_bytes().to_owned()),
                _ => None,
              },
              maybe_output_code_filename: None,
              maybe_source_map: match map["sourceMap"].as_str() {
                Some(str) => Some(str.as_bytes().to_owned()),
                _ => None,
              },
              maybe_source_map_filename: None,
            },
            _ => panic!("error decoding compiler response"),
          },
        )).unwrap(),
    )
  }));

  let worker_receiver = shared.worker_err_receiver.clone();

  let union =
    futures::future::select_all(vec![worker_receiver, local_receiver.shared()]);

  match union.wait() {
    Ok((result, i, rest)) => {
      // local_receiver finished first
      let mut rest_mut = rest;
      match ((*result.deref()).clone(), i) {
        (Ok(v), 1) => Ok(v),
        (Err(Some(err)), _) => Err(err),
        (Err(None), 1) => {
          match (*rest_mut.remove(0).wait().unwrap().deref()).clone() {
            Ok(v) => Ok(v),
            Err(Some(err)) => Err(err),
            Err(None) => panic!("Odd compiler worker result!"),
          }
        }
        (_, i) => panic!("Odd compiler result for future {}!", i),
      }
    }
    Err((err, i, _)) => panic!("compile_sync {} failed: {}", i, err),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tokio_util;

  #[test]
  fn test_compile_sync() {
    tokio_util::init(|| {
      let cwd = std::env::current_dir().unwrap();
      let cwd_string = cwd.to_str().unwrap().to_owned();

      let specifier = "./tests/002_hello.ts";
      let referrer = cwd_string + "/";

      let mut out = ModuleMetaData {
        module_name: "xxx".to_owned(),
        filename: "/tests/002_hello.ts".to_owned(),
        media_type: msg::MediaType::TypeScript,
        source_code: "console.log(\"Hello World\");".as_bytes().to_owned(),
        maybe_output_code_filename: None,
        maybe_output_code: None,
        maybe_source_map_filename: None,
        maybe_source_map: None,
      };

      let out_result = compile_sync(
        Arc::new(IsolateState::mock()),
        specifier,
        &referrer,
        &mut out,
      );
      assert!(out_result.is_ok());
      out = out_result.unwrap();
      assert!(
        out
          .maybe_output_code
          .unwrap()
          .starts_with("console.log(\"Hello World\");".as_bytes())
      );
    });
  }
}
