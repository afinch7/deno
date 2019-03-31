// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::errors::*;
use crate::isolate::{DenoBehavior, Isolate};
use crate::isolate_state::WorkerChannels;
use crate::resources;
use deno_core::Buf;
use deno_core::JSError;
use futures::sync::mpsc;
use futures::Future;
use futures::Poll;

/// Behavior trait specific to workers
pub trait WorkerBehavior: DenoBehavior {
  /// Used to setup internal channels at worker creation.
  /// This is intended to be temporary fix.
  /// TODO(afinch7) come up with a better solution to set worker channels
  fn set_internal_channels(&mut self, worker_channels: WorkerChannels);
}

/// Rust interface for WebWorkers.
pub struct Worker<B: WorkerBehavior> {
  isolate: Isolate<B>,
  pub resource: resources::Resource,
}

impl<B: WorkerBehavior> Worker<B> {
  pub fn new(mut behavior: B) -> Self {
    let (worker_in_tx, worker_in_rx) = mpsc::channel::<Buf>(1);
    let (worker_out_tx, worker_out_rx) = mpsc::channel::<Buf>(1);

    let internal_channels = (worker_out_tx, worker_in_rx);
    let external_channels = (worker_in_tx, worker_out_rx);

    behavior.set_internal_channels(internal_channels);

    let isolate = Isolate::new(behavior);

    Worker {
      isolate,
      resource: resources::add_worker(external_channels),
    }
  }

  pub fn execute(&mut self, js_source: &str) -> Result<(), JSError> {
    self.isolate.execute(js_source)
  }

  pub fn execute_mod(
    &mut self,
    js_filename: &str,
    is_prefetch: bool,
  ) -> Result<(), RustOrJsError> {
    self.isolate.execute_mod(js_filename, is_prefetch)
  }
}

impl<B: WorkerBehavior> Future for Worker<B> {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    self.isolate.poll()
  }
}

/// Method and data used to initalize a worker
pub enum WorkerInit {
  Script(String),
  Module(String),
}

pub fn spawn<B: WorkerBehavior + 'static>(
  behavior: B,
  worker_debug_name: &str,
  init: WorkerInit,
) -> Result<Worker<B>, RustOrJsError> {
  let state = behavior.state().clone();
  let mut worker = Worker::new(behavior);

  worker
    .execute(&format!("denoMain('{}')", worker_debug_name))
    .expect("worker workerInit failed");

  worker
    .execute("workerMain()")
    .expect("worker workerMain failed");

  let init_result = match init {
    WorkerInit::Script(script) => match worker.execute(&script) {
      Ok(v) => Ok(v),
      Err(e) => Err(RustOrJsError::Js(e)),
    },
    WorkerInit::Module(specifier) => {
      let should_prefetch = state.flags.prefetch || state.flags.info;
      match state.dir.resolve_module_url(&specifier, ".") {
        Err(err) => Err(RustOrJsError::Rust(DenoError::from(err))),
        Ok(module_url) => {
          worker.execute_mod(&module_url.to_string(), should_prefetch)
        }
      }
    }
  };

  match init_result {
    Ok(_) => Ok(worker),
    Err(err) => Err(err),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::compiler::CompilerBehavior;
  use crate::isolate_state::IsolateState;
  use crate::js_errors::JSErrorColor;
  use crate::tokio_util;
  use futures::future::lazy;
  use std::thread;

  #[test]
  fn test_spawn() {
    let worker_result = spawn(
      CompilerBehavior::new(
        IsolateState::mock().flags.clone(),
        IsolateState::mock().argv.clone(),
      ),
      "TEST",
      WorkerInit::Script(
        r#"
      onmessage = function(e) {
        console.log("msg from main script", e.data);
        if (e.data == "exit") {
          close();
          return;
        } else {
          console.assert(e.data === "hi");
        }
        postMessage([1, 2, 3]);
        console.log("after postMessage");
      }
      "#.into(),
      ),
    );
    assert!(worker_result.is_ok());
    let worker = worker_result.unwrap();
    let resource = worker.resource.clone();
    let resource_ = resource.clone();

    let builder = thread::Builder::new().name("test-worker".to_string());

    let _tid = builder.spawn(move || {
      tokio_util::run(lazy(move || {
        worker.then(move |r| -> Result<(), ()> {
          resource_.close();
          debug!("workers.rs after resource close");
          if let Err(err) = r {
            eprintln!("{}", JSErrorColor(&err).to_string());
            assert!(false)
          }
          Ok(())
        })
      }))
    });

    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();

    let r = resources::post_message_to_worker(resource.rid, msg).wait();
    assert!(r.is_ok());

    let maybe_msg = resources::get_message_from_worker(resource.rid)
      .wait()
      .unwrap();
    assert!(maybe_msg.is_some());
    // Check if message received is [1, 2, 3] in json
    assert_eq!(*maybe_msg.unwrap(), *b"[1,2,3]");

    let msg = json!("exit")
      .to_string()
      .into_boxed_str()
      .into_boxed_bytes();
    let r = resources::post_message_to_worker(resource.rid, msg).wait();
    assert!(r.is_ok());
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    let worker_result = spawn(
      CompilerBehavior::new(
        IsolateState::mock().flags.clone(),
        IsolateState::mock().argv.clone(),
      ),
      "TEST",
      WorkerInit::Script("onmessage = () => close();".into()),
    );
    assert!(worker_result.is_ok());
    let worker = worker_result.unwrap();
    let resource = worker.resource.clone();
    let resource_ = resource.clone();

    let builder = thread::Builder::new().name("test-worker".to_string());

    let _tid = builder.spawn(move || {
      tokio_util::run(lazy(move || {
        worker.then(move |r| -> Result<(), ()> {
          resource_.close();
          debug!("workers.rs after resource close");
          if let Err(err) = r {
            eprintln!("{}", JSErrorColor(&err).to_string());
            assert!(false)
          }
          Ok(())
        })
      }))
    });

    assert_eq!(
      resources::get_type(resource.rid),
      Some("worker".to_string())
    );

    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
    let r = resources::post_message_to_worker(resource.rid, msg).wait();
    assert!(r.is_ok());
    println!("rid {:?}", resource.rid);

    // TODO Need a way to get a future for when a resource closes.
    // For now, just sleep for a bit.
    // resource.close();
    thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(resources::get_type(resource.rid), None);
  }
}
