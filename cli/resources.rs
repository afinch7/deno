// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno to refer to various resources.  The simplest
// example are standard file system files and stdio - but there will be other
// resources added in the future that might not correspond to operating system
// level File Descriptors. To avoid confusion we call them "resources" not "file
// descriptors". This module implements a global resource table. Ops (AKA
// handlers) look up resources by their integer id here.

use crate::deno_error::bad_resource;
use crate::http_body::HttpBody;
use deno::ErrBox;
pub use deno::Resource;
pub use deno::ResourceId;
use deno::ResourceTable;

use futures::compat::AsyncRead01CompatExt;
use futures::compat::AsyncWrite01CompatExt;
use futures::io::{AsyncRead, AsyncWrite};
use reqwest::r#async::Decoder as ReqwestDecoder;
use std;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::task::Context;
use std::task::Poll;
use tokio;
use tokio::net::TcpStream;
use tokio::prelude::Async;
use tokio_process;
use tokio_rustls::client::TlsStream as ClientTlsStream;
use tokio_rustls::server::TlsStream as ServerTlsStream;

#[cfg(not(windows))]
use std::os::unix::io::FromRawFd;

#[cfg(windows)]
use std::os::windows::io::FromRawHandle;

#[cfg(windows)]
extern crate winapi;

lazy_static! {
  static ref RESOURCE_TABLE: Mutex<ResourceTable> = Mutex::new({
    let mut table = ResourceTable::default();

    // TODO Load these lazily during lookup?
    table.add("stdin", Box::new(CliResource::Stdin(tokio::io::stdin())));

    table.add("stdout", Box::new(CliResource::Stdout({
      #[cfg(not(windows))]
      let stdout = unsafe { std::fs::File::from_raw_fd(1) };
      #[cfg(windows)]
      let stdout = unsafe {
        std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
            winapi::um::winbase::STD_OUTPUT_HANDLE))
      };
      tokio::fs::File::from_std(stdout)
    })));

    table.add("stderr", Box::new(CliResource::Stderr(tokio::io::stderr())));
    table
  });
}

// TODO: rename to `StreamResource`
pub enum CliResource {
  Stdin(tokio::io::Stdin),
  Stdout(tokio::fs::File),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File),
  TcpStream(tokio::net::TcpStream),
  ServerTlsStream(Box<ServerTlsStream<TcpStream>>),
  ClientTlsStream(Box<ClientTlsStream<TcpStream>>),
  HttpBody(Box<HttpBody>),
  ChildStdin(tokio_process::ChildStdin),
  ChildStdout(tokio_process::ChildStdout),
  ChildStderr(tokio_process::ChildStderr),
}

impl Resource for CliResource {}

pub fn lock_resource_table<'a>() -> MutexGuard<'a, ResourceTable> {
  RESOURCE_TABLE.lock().unwrap()
}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, ErrBox>>;
}

impl DenoAsyncRead for CliResource {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, ErrBox>> {
    let inner = self.get_mut();
    let mut f: Box<dyn AsyncRead + Unpin> = match inner {
      CliResource::FsFile(f) => Box::new(AsyncRead01CompatExt::compat(f)),
      CliResource::Stdin(f) => Box::new(AsyncRead01CompatExt::compat(f)),
      CliResource::TcpStream(f) => Box::new(AsyncRead01CompatExt::compat(f)),
      CliResource::ClientTlsStream(f) => {
        Box::new(AsyncRead01CompatExt::compat(f))
      }
      CliResource::ServerTlsStream(f) => {
        Box::new(AsyncRead01CompatExt::compat(f))
      }
      CliResource::HttpBody(f) => Box::new(f),
      CliResource::ChildStdout(f) => Box::new(AsyncRead01CompatExt::compat(f)),
      CliResource::ChildStderr(f) => Box::new(AsyncRead01CompatExt::compat(f)),
      _ => {
        return Poll::Ready(Err(bad_resource()));
      }
    };

    let r = AsyncRead::poll_read(Pin::new(&mut f), cx, buf);

    match r {
      Poll::Ready(Err(e)) => Poll::Ready(Err(ErrBox::from(e))),
      Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
      Poll::Pending => Poll::Pending,
    }
  }
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  //TODO(afinch7) update to behave like new AsyncWrite
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, ErrBox>>;

  fn poll_close(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Result<(), ErrBox>>;
}

impl DenoAsyncWrite for CliResource {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, ErrBox>> {
    let inner = self.get_mut();
    let mut f: Box<dyn AsyncWrite + Unpin> = match inner {
      CliResource::FsFile(f) => Box::new(AsyncWrite01CompatExt::compat(f)),
      CliResource::Stdout(f) => Box::new(AsyncWrite01CompatExt::compat(f)),
      CliResource::Stderr(f) => Box::new(AsyncWrite01CompatExt::compat(f)),
      CliResource::TcpStream(f) => Box::new(AsyncWrite01CompatExt::compat(f)),
      CliResource::ClientTlsStream(f) => {
        Box::new(AsyncWrite01CompatExt::compat(f))
      }
      CliResource::ServerTlsStream(f) => {
        Box::new(AsyncWrite01CompatExt::compat(f))
      }
      CliResource::ChildStdin(f) => Box::new(AsyncWrite01CompatExt::compat(f)),
      _ => {
        return Poll::Ready(Err(bad_resource()));
      }
    };

    let r = AsyncWrite::poll_write(Pin::new(&mut f), cx, buf);

    match r {
      Poll::Ready(Err(e)) => Poll::Ready(Err(ErrBox::from(e))),
      Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_close(
    self: Pin<&mut Self>,
    _cx: &mut Context,
  ) -> Poll<Result<(), ErrBox>> {
    unimplemented!()
  }
}

pub fn add_fs_file(fs_file: tokio::fs::File) -> ResourceId {
  let mut table = lock_resource_table();
  table.add("fsFile", Box::new(CliResource::FsFile(fs_file)))
}

pub fn add_tcp_stream(stream: tokio::net::TcpStream) -> ResourceId {
  let mut table = lock_resource_table();
  table.add("tcpStream", Box::new(CliResource::TcpStream(stream)))
}

pub fn add_tls_stream(stream: ClientTlsStream<TcpStream>) -> ResourceId {
  let mut table = lock_resource_table();
  table.add(
    "clientTlsStream",
    Box::new(CliResource::ClientTlsStream(Box::new(stream))),
  )
}

pub fn add_server_tls_stream(stream: ServerTlsStream<TcpStream>) -> ResourceId {
  let mut table = lock_resource_table();
  table.add(
    "serverTlsStream",
    Box::new(CliResource::ServerTlsStream(Box::new(stream))),
  )
}

pub fn add_reqwest_body(body: ReqwestDecoder) -> ResourceId {
  let body = HttpBody::from(body);
  let mut table = lock_resource_table();
  table.add("httpBody", Box::new(CliResource::HttpBody(Box::new(body))))
}

pub fn add_child_stdin(stdin: tokio_process::ChildStdin) -> ResourceId {
  let mut table = lock_resource_table();
  table.add("childStdin", Box::new(CliResource::ChildStdin(stdin)))
}

pub fn add_child_stdout(stdout: tokio_process::ChildStdout) -> ResourceId {
  let mut table = lock_resource_table();
  table.add("childStdout", Box::new(CliResource::ChildStdout(stdout)))
}

pub fn add_child_stderr(stderr: tokio_process::ChildStderr) -> ResourceId {
  let mut table = lock_resource_table();
  table.add("childStderr", Box::new(CliResource::ChildStderr(stderr)))
}

pub struct CloneFileFuture {
  pub rid: ResourceId,
}

impl Future for CloneFileFuture {
  type Output = Result<tokio::fs::File, ErrBox>;

  fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let mut table = lock_resource_table();
    let repr = table
      .get_mut::<CliResource>(inner.rid)
      .ok_or_else(bad_resource)?;
    match repr {
      CliResource::FsFile(ref mut file) => {
        match file.poll_try_clone().map_err(ErrBox::from) {
          Err(err) => Poll::Ready(Err(err)),
          Ok(Async::Ready(v)) => Poll::Ready(Ok(v)),
          Ok(Async::NotReady) => Poll::Pending,
        }
      }
      _ => Poll::Ready(Err(bad_resource())),
    }
  }
}
