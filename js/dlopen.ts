// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import { build } from "./build";

export type DlCallReturn = Uint8Array | undefined;

function dlCallSync(
  rid: number,
  data: Uint8Array,
  zeroCopy?: ArrayBufferView
): DlCallReturn {
  const builder = flatbuffers.createBuilder();
  const data_ = builder.createString(data);
  const inner = msg.DlCall.createDlCall(builder, rid, data_);
  const baseRes = sendSync(builder, msg.Any.DlCall, inner, zeroCopy);
  assert(baseRes != null);
  assert(
    msg.Any.DlCallRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DlCallRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray === null) {
    return undefined;
  }
  return dataArray;
}

async function dlCallAsync(
  rid: number,
  data: Uint8Array,
  zeroCopy?: ArrayBufferView
): Promise<DlCallReturn> {
  const builder = flatbuffers.createBuilder();
  const data_ = builder.createString(data);
  const inner = msg.DlCall.createDlCall(builder, rid, data_);
  const baseRes = await sendAsync(builder, msg.Any.DlCall, inner, zeroCopy);
  assert(baseRes != null);
  assert(
    msg.Any.DlCallRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DlCallRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray === null) {
    return undefined;
  }
  return dataArray;
}

function dlSym(libId: number, name: string): number {
  const builder = flatbuffers.createBuilder();
  const name_ = builder.createString(name);
  const inner = msg.DlSym.createDlSym(builder, libId, name_);
  const baseRes = sendSync(builder, msg.Any.DlSym, inner);
  assert(baseRes != null);
  assert(
    msg.Any.DlSymRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DlSymRes();
  assert(baseRes!.inner(res) != null);
  return res.rid();
}

export interface DynamicLibOp {
  dispatchSync(data: Uint8Array, zeroCopy?: ArrayBufferView): DlCallReturn;

  dispatchAsync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<DlCallReturn>;
}

// A loaded dynamic lib function.
// Loaded functions will need to loaded and addressed by unique identifiers
// for performance, since loading a function from a library for every call
// would likely be the limiting factor for many use cases.
// @internal
class DynamicLibOpImpl implements DynamicLibOp {
  private readonly rid: number;

  constructor(dlId: number, name: string) {
    this.rid = dlSym(dlId, name);
  }

  dispatchSync(data: Uint8Array, zeroCopy?: ArrayBufferView): DlCallReturn {
    return dlCallSync(this.rid, data, zeroCopy);
  }

  async dispatchAsync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<DlCallReturn> {
    return dlCallAsync(this.rid, data, zeroCopy);
  }
}

function dlOpen(filename: string): number {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const inner = msg.DlOpen.createDlOpen(builder, filename_);
  const baseRes = sendSync(builder, msg.Any.DlOpen, inner);
  assert(baseRes != null);
  assert(
    msg.Any.DlOpenRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DlOpenRes();
  assert(baseRes!.inner(res) != null);
  return res.rid();
}

export interface DynamicLib {
  loadOp(name: string): DynamicLibOp;
}

// A loaded dynamic lib.
// Dynamic libraries need to remain loaded into memory on the rust side
// ,and then be addressed by their unique identifier to avoid loading
// the same library multiple times.
export class DynamicLibImpl implements DynamicLib {
  // unique resource identifier for the loaded dynamic lib rust side
  private readonly rid: number;
  private readonly fnMap: Map<string, DynamicLibOp> = new Map();

  // @internal
  constructor(libraryPath: string) {
    this.rid = dlOpen(libraryPath);
  }

  loadOp(name: string): DynamicLibOp {
    const cachedFn = this.fnMap.get(name);
    if (cachedFn) {
      return cachedFn;
    } else {
      const dlFn = new DynamicLibOpImpl(this.rid, name);
      this.fnMap.set(name, dlFn);
      return dlFn;
    }
  }
}

export function dlopen(filename: string): DynamicLib {
  return new DynamicLibImpl(filename);
}

export type DlNamePrefix = "lib" | "";

const dlNamePrefix = ((): DlNamePrefix => {
  switch (build.os) {
    case "linux":
    case "mac":
      return "lib";
    case "win":
    default:
      return "";
  }
})();

export type DlNameExtension = "so" | "dylib" | "dll";

const dlNameExtension = ((): DlNameExtension => {
  switch (build.os) {
    case "linux":
      return "so";
    case "mac":
      return "dylib";
    case "win":
      return "dll";
  }
})();

export function dlname(filenameBase: string): string {
  return dlNamePrefix + filenameBase + "." + dlNameExtension;
}
