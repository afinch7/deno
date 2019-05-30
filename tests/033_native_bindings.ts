const { dlname, dlopen, env } = Deno;

let localPath: any = import.meta.url.split("/");
localPath.pop();
localPath = localPath.join("/");

const dLib = dlopen(localPath + "/../target/" + env().DENO_BUILD_MODE + "/" + dlname("test_binding"));
const testOpFn = dLib.loadFn("test_op");

interface TestOptions {
    data: any;
    zeroCopyData: any;
}

interface TestResponse {
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

const testOp = (args: TestOptions): any => {
    return decodeTestOp(
        testOpFn.dispatchSync(
            encodeTestOp(args.data),
            encodeTestOp(args.zeroCopyData),
        ),
    );
}

console.log(testOp({ data: "test", zeroCopyData: { some: "data" } }));
