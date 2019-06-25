// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import { build } from "./build";

export type PluginCallReturn = Uint8Array | undefined;

function pluginCallSync(
  rid: number,
  data: Uint8Array,
  zeroCopy?: ArrayBufferView
): PluginCallReturn {
  const builder = flatbuffers.createBuilder();
  const data_ = builder.createString(data);
  const inner = msg.PluginCall.createPluginCall(builder, rid, data_);
  const baseRes = sendSync(builder, msg.Any.PluginCall, inner, zeroCopy);
  assert(baseRes != null);
  assert(
    msg.Any.PluginCallRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.PluginCallRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray === null) {
    return undefined;
  }
  return dataArray;
}

async function pluginCallAsync(
  rid: number,
  data: Uint8Array,
  zeroCopy?: ArrayBufferView
): Promise<PluginCallReturn> {
  const builder = flatbuffers.createBuilder();
  const data_ = builder.createString(data);
  const inner = msg.PluginCall.createPluginCall(builder, rid, data_);
  const baseRes = await sendAsync(builder, msg.Any.PluginCall, inner, zeroCopy);
  assert(baseRes != null);
  assert(
    msg.Any.PluginCallRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.PluginCallRes();
  assert(baseRes!.inner(res) != null);

  const dataArray = res.dataArray();
  if (dataArray === null) {
    return undefined;
  }
  return dataArray;
}

function pluginSym(libId: number, name: string): number {
  const builder = flatbuffers.createBuilder();
  const name_ = builder.createString(name);
  const inner = msg.PluginSym.createPluginSym(builder, libId, name_);
  const baseRes = sendSync(builder, msg.Any.PluginSym, inner);
  assert(baseRes != null);
  assert(
    msg.Any.PluginSymRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.PluginSymRes();
  assert(baseRes!.inner(res) != null);
  return res.rid();
}

export interface PluginOp {
  dispatchSync(data: Uint8Array, zeroCopy?: ArrayBufferView): PluginCallReturn;

  dispatchAsync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<PluginCallReturn>;
}

// A loaded dynamic lib function.
// Loaded functions will need to loaded and addressed by unique identifiers
// for performance, since loading a function from a library for every call
// would likely be the limiting factor for many use cases.
// @internal
class PluginOpImpl implements PluginOp {
  private readonly rid: number;

  constructor(dlId: number, name: string) {
    this.rid = pluginSym(dlId, name);
  }

  dispatchSync(data: Uint8Array, zeroCopy?: ArrayBufferView): PluginCallReturn {
    return pluginCallSync(this.rid, data, zeroCopy);
  }

  async dispatchAsync(
    data: Uint8Array,
    zeroCopy?: ArrayBufferView
  ): Promise<PluginCallReturn> {
    return pluginCallAsync(this.rid, data, zeroCopy);
  }
}

function dlOpen(filename: string): number {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const inner = msg.PluginOpen.createPluginOpen(builder, filename_);
  const baseRes = sendSync(builder, msg.Any.PluginOpen, inner);
  assert(baseRes != null);
  assert(
    msg.Any.PluginOpenRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const res = new msg.PluginOpenRes();
  assert(baseRes!.inner(res) != null);
  return res.rid();
}

export interface Plugin {
  loadOp(name: string): PluginOp;
}

// A loaded dynamic lib.
// Dynamic libraries need to remain loaded into memory on the rust side
// ,and then be addressed by their unique identifier to avoid loading
// the same library multiple times.
export class PluginImpl implements Plugin {
  // unique resource identifier for the loaded dynamic lib rust side
  private readonly rid: number;
  private readonly fnMap: Map<string, PluginOp> = new Map();

  // @internal
  constructor(libraryPath: string) {
    this.rid = dlOpen(libraryPath);
  }

  loadOp(name: string): PluginOp {
    const cachedFn = this.fnMap.get(name);
    if (cachedFn) {
      return cachedFn;
    } else {
      const dlFn = new PluginOpImpl(this.rid, name);
      this.fnMap.set(name, dlFn);
      return dlFn;
    }
  }
}

export function openPlugin(filename: string): Plugin {
  return new PluginImpl(filename);
}

export type PluginFilePrefix = "lib" | "";

const pluginFilePrefix = ((): PluginFilePrefix => {
  switch (build.os) {
    case "linux":
    case "mac":
      return "lib";
    case "win":
    default:
      return "";
  }
})();

export type PluginFileExtension = "so" | "dylib" | "dll";

const pluginFileExtension = ((): PluginFileExtension => {
  switch (build.os) {
    case "linux":
      return "so";
    case "mac":
      return "dylib";
    case "win":
      return "dll";
  }
})();

export function pluginFilename(filenameBase: string): string {
  return pluginFilePrefix + filenameBase + "." + pluginFileExtension;
}
