use super::dispatch_minimal::MinimalOp;
use crate::deno_error;
use crate::deno_error::bad_resource;
use crate::ops::minimal_op;
use crate::resources;
use crate::resources::CliResource;
use crate::resources::DenoAsyncRead;
use crate::resources::DenoAsyncWrite;
use crate::state::ThreadSafeState;
use deno::*;
use futures::future::FutureExt;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("read", s.core_op(minimal_op(op_read)));
  i.register_op("write", s.core_op(minimal_op(op_write)));
}

#[derive(Debug, PartialEq)]
enum IoState {
  Pending,
  Done,
}

/// Tries to read some bytes directly into the given `buf` in asynchronous
/// manner, returning a future type.
///
/// The returned future will resolve to both the I/O stream and the buffer
/// as well as the number of bytes read once the read operation is completed.
pub fn read<T>(rid: ResourceId, buf: T) -> Read<T>
where
  T: AsMut<[u8]>,
{
  Read {
    rid,
    buf,
    state: IoState::Pending,
  }
}

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
///
/// Created by the [`read`] function.
#[derive(Debug)]
pub struct Read<T> {
  rid: ResourceId,
  buf: T,
  state: IoState,
}

impl<T> Future for Read<T>
where
  T: AsMut<[u8]> + Unpin,
{
  type Output = Result<i32, ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    if inner.state == IoState::Done {
      panic!("poll a Read after it's done");
    }

    let mut table = resources::lock_resource_table();
    let resource = table
      .get_mut::<CliResource>(inner.rid)
      .ok_or_else(bad_resource)?;
    let nread = match DenoAsyncRead::poll_read(
      Pin::new(resource),
      cx,
      &mut inner.buf.as_mut()[..],
    ) {
      Poll::Ready(Ok(v)) => v,
      Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
      Poll::Pending => return Poll::Pending,
    };
    inner.state = IoState::Done;
    Poll::Ready(Ok(nread as i32))
  }
}

pub fn op_read(rid: i32, zero_copy: Option<PinnedBuf>) -> Pin<Box<MinimalOp>> {
  debug!("read rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return futures::future::err(deno_error::no_buffer_specified()).boxed()
    }
    Some(buf) => buf,
  };

  let fut = read(rid as u32, zero_copy);

  fut.boxed()
}

/// A future used to write some data to a stream.
#[derive(Debug)]
pub struct Write<T> {
  rid: ResourceId,
  buf: T,
  state: IoState,
}

/// Creates a future that will write some of the buffer `buf` to
/// the stream resource with `rid`.
///
/// Any error which happens during writing will cause both the stream and the
/// buffer to get destroyed.
pub fn write<T>(rid: ResourceId, buf: T) -> Write<T>
where
  T: AsRef<[u8]>,
{
  Write {
    rid,
    buf,
    state: IoState::Pending,
  }
}

/// This is almost the same implementation as in tokio, difference is
/// that error type is `ErrBox` instead of `std::io::Error`.
impl<T> Future for Write<T>
where
  T: AsRef<[u8]> + Unpin,
{
  type Output = Result<i32, ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    if inner.state == IoState::Done {
      panic!("poll a Read after it's done");
    }

    let mut table = resources::lock_resource_table();
    let resource = table
      .get_mut::<CliResource>(inner.rid)
      .ok_or_else(bad_resource)?;
    let nwritten = match DenoAsyncWrite::poll_write(
      Pin::new(resource),
      cx,
      inner.buf.as_ref(),
    ) {
      Poll::Ready(Ok(v)) => v,
      Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
      Poll::Pending => return Poll::Pending,
    };
    inner.state = IoState::Done;
    Poll::Ready(Ok(nwritten as i32))
  }
}

pub fn op_write(rid: i32, zero_copy: Option<PinnedBuf>) -> Pin<Box<MinimalOp>> {
  debug!("write rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return futures::future::err(deno_error::no_buffer_specified()).boxed()
    }
    Some(buf) => buf,
  };

  let fut = write(rid as u32, zero_copy);

  fut.boxed()
}
