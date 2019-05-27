// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::errors::{BindingResult};
use crate::dispatch::{OpDispatchFn};
use std::any::Any;
use std::fmt::Debug;

/// Initlization context with various init specific bindings
pub trait BindingInitContext {
    /// Registers a new op with the runtime. This should provide a unique OpId 
    /// at Deno.nativeBindings.opIds.$plugin_name.$name at runtime for the 
    /// ts/js side code to use.
    fn register_op(
        &self,
        name: String,
        dispatch: OpDispatchFn
    ) -> BindingResult<()>;
}

// Plugin system based off of https://michael-f-bryan.github.io/rust-ffi-guide/dynamic_loading.html

pub trait BindingPlugin: Any + Send + Sync + Debug {
    /// Get a name for debug usage.
    /// This should return a static value that should not change.
    fn name(&self) -> &'static str;
    /// Allow plugin to perform init by passing it a init context.
    fn init(&self, context: &BindingInitContext) -> BindingResult<()>;
    /// Get source for binding module. 
    /// This should return a static value that should not change.
    fn es_module_source(&self) -> String;
}

#[macro_export]
macro_rules! declare_binding_plugin {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _binding_plugin_create() -> *mut $crate::plugin::BindingPlugin {
            // make sure the constructor is the correct type.
            let constructor: fn() -> $plugin_type = $constructor;

            let object = constructor();
            let boxed: Box<$crate::plugin::BindingPlugin> = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}
