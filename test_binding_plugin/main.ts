// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

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
        Deno.nativeBindings.sendSync(
            Deno.nativeBindings.opIds.test_binding_plugin.testOp,
            encodeTestOp(args),
        ),
    );
}