const { dlname, dlopen, env } = Deno;

const dLib = dlopen(env().DENO_BUILD_PATH + "/" + dlname("test_binding"));
const testOpFn = dLib.loadOp("test_op");

interface TestOptions {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  zeroCopyData: any;
}

interface TestResponse {
  data: Uint8Array;
}

const textEncoder = new TextEncoder();

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function encodeTestOp(args: any): Uint8Array {
  return textEncoder.encode(JSON.stringify(args));
}

const textDecoder = new TextDecoder();

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function decodeTestOp(data: Uint8Array): any {
  return textDecoder.decode(data);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const testOp = (args: TestOptions): any => {
  return decodeTestOp(
    testOpFn.dispatchSync(
      encodeTestOp(args.data),
      encodeTestOp(args.zeroCopyData)
    )
  );
};

console.log(testOp({ data: "test", zeroCopyData: { some: "data" } }));
