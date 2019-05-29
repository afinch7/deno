// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { core } from "./core";
import { sendSync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import { build } from "./build";

export interface DynamicLibFn {
  dispatchSync(
    data: Uint8Array,
    zeroCopy: undefined | ArrayBufferView,
  ): Uint8Array;

  dispatchAsync(
    data: Uint8Array,
    zeroCopy: undefined | ArrayBufferView,
  ): Promise<Uint8Array>;
}

// A loaded dynamic lib function.
// Loaded functions will need to loaded and addressed by unique identifiers
// for performance, since loading a function from a library for every call 
// would likely be the limiting factor for many use cases.
/** @internal */
class DynamicLibFnImpl implements DynamicLibFn {
  
  private readonly dlFnId: number;

  constructor(dlId: number, name: string) {
    this.dlFnId = loadDlFn(dlId, name);
  }

  dispatchSync(
    data: Uint8Array,
    zeroCopy: undefined | ArrayBufferView,
  ): Uint8Array {
    // Like the prior Deno.nativeBindings.sendSync
    return callDlFnSync(this.dlFnId, data, zeroCopy);
  }

  async dispatchAsync(
    data: Uint8Array,
    zeroCopy: undefined | ArrayBufferView,
  ): Promise<Uint8Array> {
    // Like the prior Deno.nativeBindings.sendSync but async
    return callDlFnAsync(this.dlFnId, data, zeroCopy);
  }

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
  private readonly dlId: number;
  private readonly fnMap: Map<string, DynamicLibFn> = new Map();

  constructor(private readonly libraryPath: string) {
    // call some op to load the library and get a resource identifier
    this.dlId = loadDl(libraryPath);
  }

  loadFn(name: string): DynamicLibFn {
    // call some op to load the fn from this library and get resource identifier
    const cachedFn = this.fnMap.get(name);
    if (cachedFn) {
      return cachedFn;
    } else {
      const dlFn = new DynamicLibFnImpl(this.dlId, name);
      this.fnMap.set(name, dlFn);
      return dlFn;
    }
  }

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
  };
})();

export type PlatformFilenameExtension = "so" | "dylib" | "dll";

export const platformFilenameExtension = ((): PlatformFilenameExtension => {
  switch(build.os) {
    case "linux":
      return "so";
    case "mac":
      return "dylib";
    case "win":
      return "dll";
  }
})();

export function platformFilename(filenameBase: string): string {
  return platformFilenamePrefix + filenameBase + "." + platformFilenameExtension;
};