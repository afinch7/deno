// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::plugin::BindingPlugin;
use std::path::PathBuf;
use libloading::{Library, Symbol, Result};
use cargo::ops::compile;
use cargo::ops::CompileOptions;
use cargo::util::homedir;
use cargo::util::config::Config;
use cargo::core::Workspace;
use cargo::core::SourceId;
use cargo::core::shell::Shell;
use cargo::core::manifest::EitherManifest;
use cargo::core::compiler::CompileMode;
use cargo::util::toml::read_manifest;

// TODO(afinch7) move plugin loading to cli

pub unsafe fn load_plugin<P: Into<PathBuf>>(manifest_path: P) -> Result<(Vec<Box<BindingPlugin>>, Vec<Library>)> {
    type PluginCreate = unsafe fn() -> *mut BindingPlugin;
    
    let manifest_path: PathBuf = manifest_path.into();
    let mut plugin_wd = manifest_path.clone();
    plugin_wd.pop();
    // TODO(afinch7) find a way to mute shell output 
    let shell = Shell::new();
    let home_dir = homedir(&plugin_wd).unwrap();

    let config = Config::new(shell, plugin_wd.clone(), home_dir);
    let manifest = read_manifest(&manifest_path, SourceId::for_directory(&plugin_wd).unwrap(), &config).unwrap();
    let manifest = match manifest.0 {
        EitherManifest::Real(man) => man,
        _ => unimplemented!(),
    };
    let ws = Workspace::new(&manifest_path, &config).unwrap();

    let mut compile_opts = CompileOptions::new(&ws.config(), CompileMode::Build).unwrap();
    compile_opts.build_config.release = true;

    let compile_result = compile(&ws, &compile_opts).unwrap();

    let mut loaded_libraries = Vec::new();
    let mut plugins = Vec::new();
    for target in manifest.targets() {
        if target.is_lib() {
            if target.is_cdylib() {
                let lib_name = format!("lib{}.so", target.crate_name());
                let lib_path = compile_result.root_output.join(lib_name);
                println!("LIB PATH: {:#?}", lib_path);
                let lib = Library::new(lib_path).unwrap();
                // We place the loaded lib into a vec so that it's contents 
                // remain statically located in memory.
                loaded_libraries.push(lib);

                let lib = loaded_libraries.last().unwrap();

                let constructor: Symbol<PluginCreate> = lib.get(b"_binding_plugin_create")
                    .unwrap();
                let boxed_raw = constructor();
                let plugin = Box::from_raw(boxed_raw);

                println!("Loaded plugin: {}", plugin.name());

                plugins.push(plugin);
            }
        }
    }
    Ok((plugins, loaded_libraries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{OpId, BindingInitContext};
    use crate::dispatch::{OpDispatchFn, BindingDispatchContext, Future};
    use crate::errors::BindingResult;
    use std::env;
    use std::sync::Mutex;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    lazy_static! {
        static ref NEXT_OP_ID: AtomicU32 = AtomicU32::new(0);
        static ref OP_ID_TABLE: Mutex<HashMap<OpId, OpDispatchFn>> = Mutex::new(HashMap::new());
        static ref V8_OP_ID_MOCK_TABLE: Mutex<HashMap<String, OpId>> = Mutex::new(HashMap::new());
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
            MockInitContext {
                plugin_name,
            }
        }
    }

    impl BindingInitContext for MockInitContext {
        fn register_op(&self, name: String, dispatch: OpDispatchFn) -> BindingResult<()> {
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

    impl BindingDispatchContext for MockDispatchContext {
    }

    #[test]
    fn test_loader() {
        println!("CWD {:#?}", env::current_dir().unwrap());
        let (plugins, _libraries) = unsafe {
            load_plugin(env::current_dir().unwrap().join("test_binding_plugin/Cargo.toml")).unwrap()
        };

        let dispatch_ctx = MockDispatchContext::new();
        for plugin in plugins {
            let next_op_id = NEXT_OP_ID.load(Ordering::SeqCst);
            let plugin_name = plugin.name().to_string();

            let init_ctx = MockInitContext::new(plugin_name.clone());
            plugin.init(&init_ctx).unwrap();

            let v8_op_id = get_op_id_from_v8_mock(&format!("{}.{}", plugin_name, "testOp"));
            assert!(v8_op_id.is_some());
            let v8_op_id_num = v8_op_id.unwrap();
            assert_eq!(v8_op_id_num, next_op_id);

            let op_id_table = OP_ID_TABLE.lock().unwrap();
            let op_dispatch = op_id_table.get(&next_op_id);
            assert!(op_dispatch.is_some());
            let result_future = op_dispatch.unwrap()(&dispatch_ctx, "some disptach text", None);
            let result = result_future.wait();
            assert!(result.is_ok());
            let result_buf = result.unwrap();
            assert_eq!(*result_buf, *b"test")
        }
    }
}