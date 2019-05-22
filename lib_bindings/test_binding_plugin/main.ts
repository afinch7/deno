// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

export interface TestOptions {
    name: string;
}

export interface TestResponse {
    data: string;
}

function encodeTestOp(_args: TestOptions): Uint8Array {
    // Do some encoding
    return new Uint8Array(0);
}

function decodeTestOpResponse(_response: Uint8Array): TestResponse {
    // Do some decoding
    return { data: "test" };
}

export const testOp = (args: TestOptions): TestResponse => {
    return decodeTestOpResponse(sendSync(Deno.opIds.test_binding_plugin.testOp, encodeTestOp(args)));
}