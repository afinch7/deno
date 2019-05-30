// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::errors::DenoResult;
use dlopen::symbor::Library;
use std::ffi::OsStr;

pub type DylibId = u32;

pub type DylibFnId = u32;

// Plugin system based off of https://michael-f-bryan.github.io/rust-ffi-guide/dynamic_loading.html

pub fn load_binding<P: AsRef<OsStr>>(lib_path: P) -> DenoResult<Library> {
  debug!("LOADING NATIVE BINDING LIB: {:#?}", lib_path.as_ref());

  let lib = Library::open(lib_path)?;

  Ok(lib)
}

// TODO(afinch7) add some tests back here
