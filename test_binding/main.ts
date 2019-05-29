// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { platformFilename, loadDylib } = Deno;

const dLib = loadDylib("target/release/" + platformFilename("test_binding"));
const testOpFn = dLib.loadFn("test_op");

export interface TestOptions {
    data: any;
    zeroCopyData: any;
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
            encodeTestOp(args.data),
            encodeTestOp(args.zeroCopyData),
        ),
    );
}