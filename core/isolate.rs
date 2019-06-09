// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Do not add dependenies to modules.rs. it should remain decoupled from the
// isolate to keep the Isolate struct from becoming too bloating for users who
// do not need asynchronous module loading.

use crate::js_errors::JSError;
use crate::libdeno;
use crate::libdeno::deno_buf;
use crate::libdeno::deno_mod;
use crate::libdeno::deno_pinned_buf;
use crate::libdeno::PinnedBuf;
use crate::libdeno::Snapshot1;
use crate::libdeno::Snapshot2;
use crate::shared_queue::SharedQueue;
use crate::shared_queue::RECOMMENDED_SIZE;
use futures::stream::{FuturesUnordered, Stream};
use futures::task;
use futures::Async::*;
use futures::Future;
use futures::Poll;
use libc::c_int;
use libc::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::ptr::null;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, Once, ONCE_INIT};

pub type Buf = Box<[u8]>;

pub type OpAsyncFuture<I, E> = Box<dyn Future<Item = I, Error = E> + Send>;

pub enum Op<E> {
  Sync(Buf),
  Async(OpAsyncFuture<Buf, E>),
}

pub type CoreError = ();

type CoreOpAsyncFuture = OpAsyncFuture<(c_int, Buf), CoreError>;

pub type CoreOp = Op<CoreError>;

/// Stores a script used to initalize a Isolate
pub struct Script<'a> {
  pub source: &'a str,
  pub filename: &'a str,
}

/// Represents data used to initialize isolate at startup
/// either a binary snapshot or a javascript source file
/// in the form of the StartupScript struct.
pub enum StartupData<'a> {
  Script(Script<'a>),
  Snapshot(&'a [u8]),
  LibdenoSnapshot(Snapshot1<'a>),
  None,
}

pub type OpResult<E> = Result<Op<E>, E>;

type CoreDispatchFn = Fn(&[u8], Option<PinnedBuf>) -> CoreOp;

#[derive(Default)]
pub struct Config {
  dispatch: Option<Arc<CoreDispatchFn>>,
  pub will_snapshot: bool,
}

impl Config {
  /// Defines the how Deno.core.dispatch() acts.
  /// Called whenever Deno.core.dispatch() is called in JavaScript. zero_copy_buf
  /// corresponds to the second argument of Deno.core.dispatch().
  pub fn dispatch<F>(&mut self, f: F)
  where
    F: Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static,
  {
    self.dispatch = Some(Arc::new(f));
  }
}

enum ResponseData {
  None,
  Buffer(deno_buf),
  PromiseId(c_int),
}

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM. An Isolate is a Future that can be used with
/// Tokio.  The Isolate future complete when there is an error or when all
/// pending ops have completed.
///
/// Ops are created in JavaScript by calling Deno.core.dispatch(), and in Rust
/// by implementing deno::Dispatch::dispatch. An async Op corresponds exactly to
/// a Promise in JavaScript.
pub struct Isolate {
  libdeno_isolate: *const libdeno::isolate,
  shared_libdeno_isolate: Arc<Mutex<Option<*const libdeno::isolate>>>,
  config: Config,
  needs_init: bool,
  shared: SharedQueue,
  pending_ops: FuturesUnordered<CoreOpAsyncFuture>,
  have_unpolled_ops: bool,
  next_promise_id: AtomicI32,
}

unsafe impl Send for Isolate {}

impl Drop for Isolate {
  fn drop(&mut self) {
    // remove shared_libdeno_isolate reference
    *self.shared_libdeno_isolate.lock().unwrap() = None;

    unsafe { libdeno::deno_delete(self.libdeno_isolate) }
  }
}

static DENO_INIT: Once = ONCE_INIT;

impl Isolate {
  /// startup_data defines the snapshot or script used at startup to initalize
  /// the isolate.
  // TODO(ry) move startup_data into Config. Ideally without introducing a
  // generic lifetime into the Isolate struct...
  pub fn new(startup_data: StartupData, config: Config) -> Self {
    DENO_INIT.call_once(|| {
      unsafe { libdeno::deno_init() };
    });

    let shared = SharedQueue::new(RECOMMENDED_SIZE);

    let needs_init = true;

    let mut startup_script: Option<Script> = None;
    let mut libdeno_config = libdeno::deno_config {
      will_snapshot: if config.will_snapshot { 1 } else { 0 },
      load_snapshot: Snapshot2::empty(),
      shared: shared.as_deno_buf(),
      recv_cb: Self::pre_dispatch,
    };

    // Seperate into Option values for each startup type
    match startup_data {
      StartupData::Script(d) => {
        startup_script = Some(d);
      }
      StartupData::Snapshot(d) => {
        libdeno_config.load_snapshot = d.into();
      }
      StartupData::LibdenoSnapshot(d) => {
        libdeno_config.load_snapshot = d;
      }
      StartupData::None => {}
    };

    let libdeno_isolate = unsafe { libdeno::deno_new(libdeno_config) };

    let mut core_isolate = Self {
      libdeno_isolate,
      shared_libdeno_isolate: Arc::new(Mutex::new(Some(libdeno_isolate))),
      config,
      shared,
      needs_init,
      pending_ops: FuturesUnordered::new(),
      have_unpolled_ops: false,
      next_promise_id: AtomicI32::new(1),
    };

    // If we want to use execute this has to happen here sadly.
    if let Some(s) = startup_script {
      core_isolate.execute(s.filename, s.source).unwrap()
    };

    core_isolate
  }

  /// Get a thread safe handle on the isolate.
  pub fn shared_isolate_handle(&mut self) -> IsolateHandle {
    IsolateHandle {
      shared_libdeno_isolate: self.shared_libdeno_isolate.clone(),
    }
  }

  /// Executes a bit of built-in JavaScript to provide Deno.sharedQueue.
  pub fn shared_init(&mut self) {
    if self.needs_init {
      self.needs_init = false;
      js_check(
        self.execute("shared_queue.js", include_str!("shared_queue.js")),
      );
    }
  }

  pub fn get_next_promise_id(&self) -> i32 {
    self.next_promise_id.fetch_add(1, Ordering::SeqCst)
  }

  extern "C" fn pre_dispatch(
    user_data: *mut c_void,
    control_argv0: deno_buf,
    zero_copy_buf: deno_pinned_buf,
  ) {
    let isolate = unsafe { Isolate::from_raw_ptr(user_data) };
    let control_shared = isolate.shared.shift();

    let op = if control_argv0.len() > 0 {
      // The user called Deno.core.send(control)
      if let Some(ref f) = isolate.config.dispatch {
        f(control_argv0.as_ref(), PinnedBuf::new(zero_copy_buf))
      } else {
        panic!("isolate.config.dispatch not set")
      }
    } else if let Some(c) = control_shared {
      // The user called Deno.sharedQueue.push(control)
      if let Some(ref f) = isolate.config.dispatch {
        f(&c, PinnedBuf::new(zero_copy_buf))
      } else {
        panic!("isolate.config.dispatch not set")
      }
    } else {
      // The sharedQueue is empty. The shouldn't happen usually, but it's also
      // not technically a failure.
      #[cfg(test)]
      unreachable!();
      #[cfg(not(test))]
      return;
    };

    // At this point the SharedQueue should be empty.
    assert_eq!(isolate.shared.size(), 0);

    match op {
      Op::Sync(buf) => {
        // For sync messages, we always return the response via Deno.core.send's
        // return value.
        // TODO(ry) check that if JSError thrown during respond(), that it will be
        // picked up.
        let _ =
          isolate.respond(ResponseData::Buffer(deno_buf::from(buf.as_ref())));
      }
      Op::Async(fut) => {
        let promise_id = isolate.get_next_promise_id();
        let _ = isolate.respond(ResponseData::PromiseId(promise_id));
        let fut = Box::new(fut.and_then(
          move |buf| -> Result<(c_int, Buf), CoreError> {
            Ok((promise_id, buf))
          },
        ));
        isolate.pending_ops.push(fut);
        isolate.have_unpolled_ops = true;
      }
    }
  }

  #[inline]
  unsafe fn from_raw_ptr<'a>(ptr: *const c_void) -> &'a mut Self {
    let ptr = ptr as *mut _;
    &mut *ptr
  }

  #[inline]
  fn as_raw_ptr(&self) -> *const c_void {
    self as *const _ as *const c_void
  }

  pub fn execute(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), JSError> {
    self.shared_init();
    let filename = CString::new(js_filename).unwrap();
    let source = CString::new(js_source).unwrap();
    unsafe {
      libdeno::deno_execute(
        self.libdeno_isolate,
        self.as_raw_ptr(),
        filename.as_ptr(),
        source.as_ptr(),
      )
    };
    if let Some(err) = self.last_exception() {
      return Err(err);
    }
    Ok(())
  }

  fn last_exception(&self) -> Option<JSError> {
    let ptr = unsafe { libdeno::deno_last_exception(self.libdeno_isolate) };
    if ptr.is_null() {
      None
    } else {
      let cstr = unsafe { CStr::from_ptr(ptr) };
      let v8_exception = cstr.to_str().unwrap();
      debug!("v8_exception\n{}\n", v8_exception);
      let js_error = JSError::from_v8_exception(v8_exception).unwrap();
      Some(js_error)
    }
  }

  fn check_promise_errors(&self) {
    unsafe {
      libdeno::deno_check_promise_errors(self.libdeno_isolate);
    }
  }

  // the result type is a placeholder for a more specific enum type
  fn respond(&mut self, data: ResponseData) -> Result<(), JSError> {
    match data {
      ResponseData::PromiseId(pid) => unsafe {
        libdeno::deno_respond(
          self.libdeno_isolate,
          self.as_raw_ptr(),
          deno_buf::empty(),
          &pid,
        )
      },
      ResponseData::Buffer(r) => unsafe {
        libdeno::deno_respond(
          self.libdeno_isolate,
          self.as_raw_ptr(),
          r,
          null(),
        )
      },
      ResponseData::None => unsafe {
        libdeno::deno_respond(
          self.libdeno_isolate,
          self.as_raw_ptr(),
          deno_buf::empty(),
          null(),
        )
      },
    };
    if let Some(err) = self.last_exception() {
      Err(err)
    } else {
      Ok(())
    }
  }

  /// Low-level module creation.
  pub fn mod_new(
    &self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<deno_mod, JSError> {
    let name_ = CString::new(name.to_string()).unwrap();
    let name_ptr = name_.as_ptr() as *const libc::c_char;

    let source_ = CString::new(source.to_string()).unwrap();
    let source_ptr = source_.as_ptr() as *const libc::c_char;

    let id = unsafe {
      libdeno::deno_mod_new(self.libdeno_isolate, main, name_ptr, source_ptr)
    };
    if let Some(js_error) = self.last_exception() {
      assert_eq!(id, 0);
      return Err(js_error);
    }

    Ok(id)
  }

  pub fn mod_get_imports(&self, id: deno_mod) -> Vec<String> {
    let len =
      unsafe { libdeno::deno_mod_imports_len(self.libdeno_isolate, id) };
    let mut out = Vec::new();
    for i in 0..len {
      let specifier_ptr =
        unsafe { libdeno::deno_mod_imports_get(self.libdeno_isolate, id, i) };
      let specifier_c: &CStr = unsafe { CStr::from_ptr(specifier_ptr) };
      let specifier: &str = specifier_c.to_str().unwrap();

      out.push(specifier.to_string());
    }
    out
  }

  pub fn snapshot(&self) -> Result<Snapshot1<'static>, JSError> {
    let snapshot = unsafe { libdeno::deno_snapshot_new(self.libdeno_isolate) };
    if let Some(js_error) = self.last_exception() {
      assert_eq!(snapshot.data_ptr, null());
      assert_eq!(snapshot.data_len, 0);
      return Err(js_error);
    }
    assert_ne!(snapshot.data_ptr, null());
    assert_ne!(snapshot.data_len, 0);
    Ok(snapshot)
  }
}

/// Called during mod_instantiate() to resolve imports.
type ResolveFn<'a> = dyn FnMut(&str, deno_mod) -> deno_mod + 'a;

/// Used internally by Isolate::mod_instantiate to wrap ResolveFn and
/// encapsulate pointer casts.
struct ResolveContext<'a> {
  resolve_fn: &'a mut ResolveFn<'a>,
}

impl<'a> ResolveContext<'a> {
  #[inline]
  fn as_raw_ptr(&mut self) -> *mut c_void {
    self as *mut _ as *mut c_void
  }

  #[inline]
  unsafe fn from_raw_ptr(ptr: *mut c_void) -> &'a mut Self {
    &mut *(ptr as *mut _)
  }
}

impl Isolate {
  pub fn mod_instantiate(
    &mut self,
    id: deno_mod,
    resolve_fn: &mut ResolveFn,
  ) -> Result<(), JSError> {
    let libdeno_isolate = self.libdeno_isolate;
    let mut ctx = ResolveContext { resolve_fn };
    unsafe {
      libdeno::deno_mod_instantiate(
        libdeno_isolate,
        ctx.as_raw_ptr(),
        id,
        Self::resolve_cb,
      )
    };

    if let Some(js_error) = self.last_exception() {
      return Err(js_error);
    }
    Ok(())
  }

  /// Called during mod_instantiate() only.
  extern "C" fn resolve_cb(
    user_data: *mut libc::c_void,
    specifier_ptr: *const libc::c_char,
    referrer: deno_mod,
  ) -> deno_mod {
    let ResolveContext { resolve_fn } =
      unsafe { ResolveContext::from_raw_ptr(user_data) };
    let specifier_c: &CStr = unsafe { CStr::from_ptr(specifier_ptr) };
    let specifier: &str = specifier_c.to_str().unwrap();

    resolve_fn(specifier, referrer)
  }

  pub fn mod_evaluate(&mut self, id: deno_mod) -> Result<(), JSError> {
    self.shared_init();
    unsafe {
      libdeno::deno_mod_evaluate(self.libdeno_isolate, self.as_raw_ptr(), id)
    };
    if let Some(js_error) = self.last_exception() {
      return Err(js_error);
    }
    Ok(())
  }
}

struct LockerScope {
  libdeno_isolate: *const libdeno::isolate,
}

impl LockerScope {
  fn new(libdeno_isolate: *const libdeno::isolate) -> LockerScope {
    unsafe { libdeno::deno_lock(libdeno_isolate) }
    LockerScope { libdeno_isolate }
  }
}

impl Drop for LockerScope {
  fn drop(&mut self) {
    unsafe { libdeno::deno_unlock(self.libdeno_isolate) }
  }
}

impl Future for Isolate {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    // Lock the current thread for V8.
    let _locker = LockerScope::new(self.libdeno_isolate);

    let mut overflow_response: Option<(c_int, Buf)> = None;

    loop {
      self.have_unpolled_ops = false;
      #[allow(clippy::match_wild_err_arm)]
      match self.pending_ops.poll() {
        Err(_) => panic!("unexpected op error"),
        Ok(Ready(None)) => break,
        Ok(NotReady) => break,
        Ok(Ready(Some(op))) => {
          let successful_push = self.shared.push(op.0, &op.1);
          if !successful_push {
            // If we couldn't push the response to the shared queue, because
            // there wasn't enough size, we will return the buffer via the
            // legacy route, using the argument of deno_respond.
            overflow_response = Some(op);
            break;
          }
        }
      }
    }

    if self.shared.size() > 0 {
      self.respond(ResponseData::None)?;
      // The other side should have shifted off all the messages.
      assert_eq!(self.shared.size(), 0);
    }

    if overflow_response.is_some() {
      let op = overflow_response.take().unwrap();
      let promise_id_bytes = op.0.to_be_bytes();
      let buf: Buf = [&promise_id_bytes, &op.1[..]].concat().into();
      self.respond(ResponseData::Buffer(deno_buf::from(buf.as_ref())))?;
    }

    self.check_promise_errors();
    if let Some(err) = self.last_exception() {
      return Err(err);
    }

    // We're idle if pending_ops is empty.
    if self.pending_ops.is_empty() {
      Ok(futures::Async::Ready(()))
    } else {
      if self.have_unpolled_ops {
        task::current().notify();
      }
      Ok(futures::Async::NotReady)
    }
  }
}

/// IsolateHandle is a thread safe handle on an Isolate. It exposed thread safe V8 functions.
#[derive(Clone)]
pub struct IsolateHandle {
  shared_libdeno_isolate: Arc<Mutex<Option<*const libdeno::isolate>>>,
}

unsafe impl Send for IsolateHandle {}

impl IsolateHandle {
  /// Terminate the execution of any currently running javascript.
  /// After terminating execution it is probably not wise to continue using
  /// the isolate.
  pub fn terminate_execution(&self) {
    unsafe {
      if let Some(isolate) = *self.shared_libdeno_isolate.lock().unwrap() {
        libdeno::deno_terminate_execution(isolate)
      }
    }
  }
}

pub fn js_check(r: Result<(), JSError>) {
  if let Err(e) = r {
    panic!(e.to_string());
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use futures::executor::spawn;
  use futures::future::lazy;
  use futures::future::ok;
  use futures::Async;
  use std::ops::FnOnce;
  use std::sync::atomic::{AtomicUsize, Ordering};

  fn run_in_task<F, R>(f: F) -> R
  where
    F: FnOnce() -> R,
  {
    spawn(lazy(move || ok::<R, ()>(f()))).wait_future().unwrap()
  }

  fn poll_until_ready<F>(
    future: &mut F,
    max_poll_count: usize,
  ) -> Result<F::Item, F::Error>
  where
    F: Future,
  {
    for _ in 0..max_poll_count {
      match future.poll() {
        Ok(NotReady) => continue,
        Ok(Ready(val)) => return Ok(val),
        Err(err) => return Err(err),
      }
    }
    panic!(
      "Isolate still not ready after polling {} times.",
      max_poll_count
    )
  }

  pub enum Mode {
    AsyncImmediate,
    OverflowReqSync,
    OverflowResSync,
    OverflowReqAsync,
    OverflowResAsync,
  }

  pub fn setup(mode: Mode) -> (Isolate, Arc<AtomicUsize>) {
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let mut config = Config::default();
    config.dispatch(move |control: &[u8], _| -> CoreOp {
      dispatch_count_.fetch_add(1, Ordering::Relaxed);
      match mode {
        Mode::AsyncImmediate => {
          assert_eq!(control.len(), 1);
          assert_eq!(control[0], 42);
          let buf = vec![43u8].into_boxed_slice();
          Op::Async(Box::new(futures::future::ok(buf)))
        }
        Mode::OverflowReqSync => {
          assert_eq!(control.len(), 100 * 1024 * 1024);
          let buf = vec![43u8].into_boxed_slice();
          Op::Sync(buf)
        }
        Mode::OverflowResSync => {
          assert_eq!(control.len(), 1);
          assert_eq!(control[0], 42);
          let mut vec = Vec::<u8>::new();
          vec.resize(100 * 1024 * 1024, 0);
          vec[0] = 99;
          let buf = vec.into_boxed_slice();
          Op::Sync(buf)
        }
        Mode::OverflowReqAsync => {
          assert_eq!(control.len(), 100 * 1024 * 1024);
          let buf = vec![43u8].into_boxed_slice();
          Op::Async(Box::new(futures::future::ok(buf)))
        }
        Mode::OverflowResAsync => {
          assert_eq!(control.len(), 1);
          assert_eq!(control[0], 42);
          let mut vec = Vec::<u8>::new();
          vec.resize(100 * 1024 * 1024, 0);
          vec[0] = 4;
          let buf = vec.into_boxed_slice();
          Op::Async(Box::new(futures::future::ok(buf)))
        }
      }
    });

    let mut isolate = Isolate::new(StartupData::None, config);
    js_check(isolate.execute(
      "setup.js",
      r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    (isolate, dispatch_count)
  }

  #[test]
  fn test_dispatch() {
    let (mut isolate, dispatch_count) = setup(Mode::AsyncImmediate);
    js_check(isolate.execute(
      "filename.js",
      r#"
        let control = new Uint8Array([42]);
        Deno.core.send(control);
        async function main() {
          Deno.core.send(control);
        }
        main();
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
  }

  #[test]
  fn test_mods() {
    let (mut isolate, dispatch_count) = setup(Mode::AsyncImmediate);
    let mod_a = isolate
      .mod_new(
        true,
        "a.js",
        r#"
        import { b } from 'b.js'
        if (b() != 'b') throw Error();
        let control = new Uint8Array([42]);
        Deno.core.send(control);
      "#,
      ).unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let imports = isolate.mod_get_imports(mod_a);
    assert_eq!(imports, vec!["b.js".to_string()]);

    let mod_b = isolate
      .mod_new(false, "b.js", "export function b() { return 'b' }")
      .unwrap();
    let imports = isolate.mod_get_imports(mod_b);
    assert_eq!(imports.len(), 0);

    let resolve_count = Arc::new(AtomicUsize::new(0));
    let resolve_count_ = resolve_count.clone();

    let mut resolve = move |specifier: &str, _referrer: deno_mod| -> deno_mod {
      resolve_count_.fetch_add(1, Ordering::SeqCst);
      assert_eq!(specifier, "b.js");
      mod_b
    };

    js_check(isolate.mod_instantiate(mod_b, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 0);

    js_check(isolate.mod_instantiate(mod_a, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    js_check(isolate.mod_evaluate(mod_a));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn test_poll_async_immediate_ops() {
    run_in_task(|| {
      let (mut isolate, dispatch_count) = setup(Mode::AsyncImmediate);

      js_check(isolate.execute(
        "setup2.js",
        r#"
        let nrecv = 0;
        Deno.core.setAsyncHandler((buf) => {
          nrecv++;
        });
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
      js_check(isolate.execute(
        "check1.js",
        r#"
        assert(nrecv == 0);
        let control = new Uint8Array([42]);
        Deno.core.send(control);
        assert(nrecv == 0);
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert_eq!(Ok(Async::Ready(())), isolate.poll());
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      js_check(isolate.execute(
        "check2.js",
        r#"
        assert(nrecv == 1);
        Deno.core.send(control);
        assert(nrecv == 1);
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      assert_eq!(Ok(Async::Ready(())), isolate.poll());
      js_check(isolate.execute("check3.js", "assert(nrecv == 2)"));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      // We are idle, so the next poll should be the last.
      assert_eq!(Ok(Async::Ready(())), isolate.poll());
    });
  }

  #[test]
  fn test_shared() {
    run_in_task(|| {
      let (mut isolate, dispatch_count) = setup(Mode::AsyncImmediate);

      js_check(isolate.execute(
        "setup2.js",
        r#"
        let nrecv = 0;
        Deno.core.setAsyncHandler((promiseId, buf) => {
          assert(buf.byteLength === 1);
          assert(buf[0] === 43);
          nrecv++;
        });
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

      js_check(isolate.execute(
        "send1.js",
        r#"
        let control = new Uint8Array([42]);
        Deno.core.sharedQueue.push(control);
        Deno.core.send();
        assert(nrecv === 0);

        Deno.core.sharedQueue.push(control);
        Deno.core.send();
        assert(nrecv === 0);
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      assert_eq!(Ok(Async::Ready(())), isolate.poll());
      js_check(isolate.execute("send1.js", "assert(nrecv === 2);"));
    });
  }

  #[test]
  fn terminate_execution() {
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let tx_clone = tx.clone();

    let (mut isolate, _dispatch_count) = setup(Mode::AsyncImmediate);
    let shared = isolate.shared_isolate_handle();

    let t1 = std::thread::spawn(move || {
      // allow deno to boot and run
      std::thread::sleep(std::time::Duration::from_millis(100));

      // terminate execution
      shared.terminate_execution();

      // allow shutdown
      std::thread::sleep(std::time::Duration::from_millis(100));

      // unless reported otherwise the test should fail after this point
      tx_clone.send(false).ok();
    });

    let t2 = std::thread::spawn(move || {
      // run an infinite loop
      let res = isolate.execute(
        "infinite_loop.js",
        r#"
          let i = 0;
          while (true) { i++; }
        "#,
      );

      // execute() terminated, which means terminate_execution() was successful.
      tx.send(true).ok();

      if let Err(e) = res {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated");
      } else {
        panic!("should return an error");
      }

      // make sure the isolate is still unusable
      let res = isolate.execute("simple.js", "1+1;");
      if let Err(e) = res {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated");
      } else {
        panic!("should return an error");
      }
    });

    if !rx.recv().unwrap() {
      panic!("should have terminated")
    }

    t1.join().unwrap();
    t2.join().unwrap();
  }

  #[test]
  fn dangling_shared_isolate() {
    let shared = {
      // isolate is dropped at the end of this block
      let (mut isolate, _dispatch_count) = setup(Mode::AsyncImmediate);
      isolate.shared_isolate_handle()
    };

    // this should not SEGFAULT
    shared.terminate_execution();
  }

  #[test]
  fn overflow_req_sync() {
    let (mut isolate, dispatch_count) = setup(Mode::OverflowReqSync);
    js_check(isolate.execute(
      "overflow_req_sync.js",
      r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((promiseId, buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array(100 * 1024 * 1024);
        let response = Deno.core.dispatch(control);
        assert(response instanceof Uint8Array);
        assert(response.length == 1);
        assert(response[0] == 43);
        assert(asyncRecv == 0);
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_res_sync() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    let (mut isolate, dispatch_count) = setup(Mode::OverflowResSync);
    js_check(isolate.execute(
      "overflow_res_sync.js",
      r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((promiseId, buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array([42]);
        let response = Deno.core.dispatch(control);
        assert(response instanceof Uint8Array);
        assert(response.length == 100 * 1024 * 1024);
        assert(response[0] == 99);
        assert(asyncRecv == 0);
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_req_async() {
    run_in_task(|| {
      let (mut isolate, dispatch_count) = setup(Mode::OverflowReqAsync);
      js_check(isolate.execute(
        "overflow_req_async.js",
        r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((cmdId, buf) => {
          assert(buf.byteLength === 1);
          assert(buf[0] === 43);
          asyncRecv++;
        });
        // Large message that will overflow the shared space.
        let control = new Uint8Array(100 * 1024 * 1024);
        let response = Deno.core.dispatch(control);
        // Async messages always have null response.
        assert(typeof response == "number");
        assert(asyncRecv == 0);
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert_eq!(Ok(Async::Ready(())), isolate.poll());
      js_check(isolate.execute("check.js", "assert(asyncRecv == 1);"));
    });
  }

  #[test]
  fn overflow_res_async() {
    run_in_task(|| {
      // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
      // should optimize this.
      let (mut isolate, dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "overflow_res_async.js",
        r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((cmdId, buf) => {
          assert(buf.byteLength === 100 * 1024 * 1024);
          assert(buf[0] === 4);
          asyncRecv++;
        });
        // Large message that will overflow the shared space.
        let control = new Uint8Array([42]);
        let response = Deno.core.dispatch(control);
        assert(typeof response == "number");
        assert(asyncRecv == 0);
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert_eq!(Ok(()), poll_until_ready(&mut isolate, 3));
      js_check(isolate.execute("check.js", "assert(asyncRecv == 1);"));
    });
  }

  #[test]
  fn overflow_res_multiple_dispatch_async() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    run_in_task(|| {
      let (mut isolate, dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "overflow_res_multiple_dispatch_async.js",
        r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((cmdId, buf) => {
          assert(buf.byteLength === 100 * 1024 * 1024);
          assert(buf[0] === 4);
          asyncRecv++;
        });
        // Large message that will overflow the shared space.
        let control = new Uint8Array([42]);
        let response = Deno.core.dispatch(control);
        assert(typeof response == "number");
        assert(asyncRecv == 0);
        // Dispatch another message to verify that pending ops
        // are done even if shared space overflows
        Deno.core.dispatch(control);
        "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      assert_eq!(Ok(()), poll_until_ready(&mut isolate, 3));
      js_check(isolate.execute("check.js", "assert(asyncRecv == 2);"));
    });
  }

  #[test]
  fn test_js() {
    run_in_task(|| {
      let (mut isolate, _dispatch_count) = setup(Mode::AsyncImmediate);
      js_check(
        isolate.execute(
          "shared_queue_test.js",
          include_str!("shared_queue_test.js"),
        ),
      );
      assert_eq!(Ok(Async::Ready(())), isolate.poll());
    });
  }

  #[test]
  fn will_snapshot() {
    let snapshot = {
      let mut config = Config::default();
      config.will_snapshot = true;
      let mut isolate = Isolate::new(StartupData::None, config);
      js_check(isolate.execute("a.js", "a = 1 + 2"));
      let s = isolate.snapshot().unwrap();
      drop(isolate);
      s
    };

    let startup_data = StartupData::LibdenoSnapshot(snapshot);
    let mut isolate2 = Isolate::new(startup_data, Config::default());
    js_check(isolate2.execute("check.js", "if (a != 3) throw Error('x')"));
  }
}
