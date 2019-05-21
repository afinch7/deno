use crate::errors::{BindingError, BindingResult};
use deno::{Buf, PinnedBuf};
use std::any::Any;
use hyper::rt::Future;

/// Dispatch context to give ops access to various context functions
pub trait DispatchContext {
    // TODO(afinch7) add dispatch context functions
}

/// Result future of a op completion
pub type OpWithError = dyn Future<Item = Buf, Error = BindingError> + Send;

/// Dispatch funciton type
/// base is a placeholder value for now not sure what we want to use there
pub type OpDispatchFn =
  fn(state: &DispatchContext, base: &str, data: Option<PinnedBuf>)
    -> Box<OpWithError>;

/// Type for uniquie identifyer of native binding ops
pub type OpId = u32;

/// Initlization context with various init specific bindings
pub trait BindingInitContext {
    /// Registers a new op with the runtime. This should provide a unique OpId 
    /// at window.opIds.$plugin_name.$name at runtime for the ts/js side code
    /// to use.
    fn register_op(
        &self,
        name: String,
        dispatch: OpDispatchFn
    ) -> BindingResult<()>;
}

pub trait BindingPlugin: Any + Send + Sync {
    /// Get a name for debug usage.
    fn name(&self) -> &'static str;
    /// Allow plugin to perform init by passing it a init context.
    fn init(&self, context: &BindingInitContext) -> BindingResult<()>;
    /// Get source for binding module
    fn get_main_module_source(&self) -> BindingResult<String>;
}

#[macro_export]
macro_rules! declare_binding_plugin {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _binding_plugin_create() -> *mut $crate::Plugin {
            // make sure the constructor is the correct type.
            let constructor: fn() -> $plugin_type = $constructor;

            let object = constructor();
            let boxed: Box<$crate::Plugin> = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}
