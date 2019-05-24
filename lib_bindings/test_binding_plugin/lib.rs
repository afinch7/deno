// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno_lib_bindings::plugin::{BindingPlugin, BindingInitContext};
use deno_lib_bindings::dispatch::{BindingDispatchContext, PinnedBuf, OpWithError, Buf};
use deno_lib_bindings::errors::BindingResult;
use futures;

#[macro_use]
extern crate deno_lib_bindings;

#[derive(Debug, Default)]
pub struct TestBindingPlugin;

impl BindingPlugin for TestBindingPlugin {
    fn name(&self) -> &'static str {
        "test_binding_plugin"
    }

    fn init(&self, context: &BindingInitContext) -> BindingResult<()> {
        println!("TEST PLUGIN INIT");
        context.register_op("testOp".to_string(), op_test_op)?;
        Ok(())
    }

    fn es_module_source(&self) -> String {
        let source_bytes = include_bytes!("./main.ts");

        std::str::from_utf8(&source_bytes[..]).unwrap().to_string()
    }
}

pub fn op_test_op(
  _state: &BindingDispatchContext,
  base: &str,
  _data: Option<PinnedBuf>,
) -> Box<OpWithError> {
    println!("BASE: {}", base);
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    Box::new(futures::future::ok(result_box))
}

declare_binding_plugin!(TestBindingPlugin, TestBindingPlugin::default);