// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import { build } from "./build";

export type DynamicLibFnCallReturn = Uint8Array | undefined;

function callDynamicLibFnSync(
  fnId: number,
  data: Uint8Array,
  zeroCopy?: ArrayBufferView
): DynamicLibFnCallReturn {
  const builder = flatbuffers.createBuilder();
  const data_ = builder.createString(data);
  const inner = msg.DynamicLibFnCall.createDynamicLibFnCall(
    builder,
    fnId,
    data_
  );
  const baseRes = sendSync(builder, msg.Any.DynamicLibFnCall, inner, zeroCopy);
  assert(baseRes != null);
  assert(
    msg.Any.DynamicLibFnCallRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DynamicLibFnCallRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray === null) {
    return undefined;
  }
  return dataArray;
}

async function callDynamicLibFnAsync(
  fnId: number,
  data: Uint8Array,
  zeroCopy?: ArrayBufferView
): Promise<DynamicLibFnCallReturn> {
  const builder = flatbuffers.createBuilder();
  const data_ = builder.createString(data);
  const inner = msg.DynamicLibFnCall.createDynamicLibFnCall(
    builder,
    fnId,
    data_
  );
  const baseRes = await sendAsync(
    builder,
    msg.Any.DynamicLibFnCall,
    inner,
    zeroCopy
  );
  assert(baseRes != null);
  assert(
    msg.Any.DynamicLibFnCallRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DynamicLibFnCallRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray === null) {
    return undefined;
  }
  return dataArray;
}

function loadDynamicLibFn(libId: number, name: string): number {
  const builder = flatbuffers.createBuilder();
  const name_ = builder.createString(name);
  const inner = msg.DynamicLibFnLoad.createDynamicLibFnLoad(
    builder,
    libId,
    name_
  );
  const baseRes = sendSync(builder, msg.Any.DynamicLibFnLoad, inner);
  assert(baseRes != null);
  assert(
    msg.Any.DynamicLibFnLoadRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DynamicLibFnLoadRes();
  assert(baseRes!.inner(res) != null);
  return res.fnId();
}

export interface DynamicLibFn {
  dispatchSync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): DynamicLibFnCallReturn;

  dispatchAsync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<DynamicLibFnCallReturn>;
}

// A loaded dynamic lib function.
// Loaded functions will need to loaded and addressed by unique identifiers
// for performance, since loading a function from a library for every call
// would likely be the limiting factor for many use cases.
// @internal
class DynamicLibFnImpl implements DynamicLibFn {
  private readonly fnId: number;

  constructor(dlId: number, name: string) {
    this.fnId = loadDynamicLibFn(dlId, name);
  }

  dispatchSync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): DynamicLibFnCallReturn {
    // Like the prior Deno.nativeBindings.sendSync
    return callDynamicLibFnSync(this.fnId, data, zeroCopy);
  }

  async dispatchAsync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<DynamicLibFnCallReturn> {
    // Like the prior Deno.nativeBindings.sendSync but async
    return callDynamicLibFnAsync(this.fnId, data, zeroCopy);
  }
}

function loadDynamicLib(filename: string): number {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const inner = msg.DynamicLibLoad.createDynamicLibLoad(builder, filename_);
  const baseRes = sendSync(builder, msg.Any.DynamicLibLoad, inner);
  assert(baseRes != null);
  assert(
    msg.Any.DynamicLibLoadRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.DynamicLibLoadRes();
  assert(baseRes!.inner(res) != null);
  return res.libId();
}

export interface DynamicLib {
  loadFn(name: string): DynamicLibFn;
}

// A loaded dynamic lib.
// Dynamic libraries need to remain loaded into memory on the rust side
// ,and then be addressed by their unique identifier to avoid loading
// the same library multiple times.
export class DynamicLibImpl implements DynamicLib {
  // unique resource identifier for the loaded dynamic lib rust side
  private readonly libId: number;
  private readonly fnMap: Map<string, DynamicLibFn> = new Map();

  // @internal
  constructor(libraryPath: string) {
    this.libId = loadDynamicLib(libraryPath);
  }

  loadFn(name: string): DynamicLibFn {
    const cachedFn = this.fnMap.get(name);
    if (cachedFn) {
      return cachedFn;
    } else {
      const dlFn = new DynamicLibFnImpl(this.libId, name);
      this.fnMap.set(name, dlFn);
      return dlFn;
    }
  }
}

export function loadDylib(filename: string): DynamicLib {
  return new DynamicLibImpl(filename);
}

export type PlatformFilePrefix = "lib" | "";

export const platformFilenamePrefix = ((): PlatformFilePrefix => {
  switch (build.os) {
    case "linux":
    case "mac":
      return "lib";
    case "win":
    default:
      return "";
  }
})();

export type PlatformFilenameExtension = "so" | "dylib" | "dll";

export const platformFilenameExtension = ((): PlatformFilenameExtension => {
  switch (build.os) {
    case "linux":
      return "so";
    case "mac":
      return "dylib";
    case "win":
      return "dll";
  }
})();

export function platformFilename(filenameBase: string): string {
  return (
    platformFilenamePrefix + filenameBase + "." + platformFilenameExtension
  );
}
