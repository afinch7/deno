// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { dlname, dlopen } = Deno;

let localPath: any = import.meta.url.split("/");
localPath.pop();
localPath = localPath.join("/");

const dLib = dlopen(env().DL_PATH_TEST_BINDING + "/" + dlname("test_plugin"));
const testOpFn = dLib.loadOp("test_op");

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