// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { platformFilename, DynamicLibary } = Deno.dlopen;

const dLib = new DynamicLibary("target/release/" + platformFilename("test_binding_lib"));
const testOpFn = dLib.loadFn("test_op");

export interface TestOptions {
    name: string;
}

export interface TestResponse {
    data: Uint8Array;
}

const textEncoder = new TextEncoder();

function encodeTestOp(args: TestOptions): Uint8Array {
    return textEncoder.encode(JSON.stringify(args));
}

const textDecoder = new TextDecoder();

function decodeTestOp(data: Uint8Array): any {
    return textDecoder.decode(data);
}

export const testOp = (args: TestOptions): any => {
    return decodeTestOp(
        testOpFn.dispatchSync(
            encodeTestOp(args),
        ),
    );
}