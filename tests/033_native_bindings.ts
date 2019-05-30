const { dlname, dlopen, env } = Deno;

const dLib = dlopen(env().DENO_BUILD_PATH + "/" + dlname("test_binding"));
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
      encodeTestOp(args.zeroCopyData)
    )
  );
};

console.log(testOp({ data: "test", zeroCopyData: { some: "data" } }));
