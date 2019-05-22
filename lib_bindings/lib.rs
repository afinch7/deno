// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
pub mod dispatch;
pub mod errors;
pub mod loader;
pub mod plugin;

#[allow(unused_imports)] 
#[macro_use]
extern crate lazy_static;

// Plugin loading system based off of https://michael-f-bryan.github.io/rust-ffi-guide/dynamic_loading.html
