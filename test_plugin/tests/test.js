const filenameBase = "test_plugin";

let filenameSuffix = ".so";
let filenamePrefix = "lib";

if (Deno.build.os === "win") {
  filenameSuffix = ".dll";
  filenamePrefix = "";
}
if (Deno.build.os === "mac") {
  filenameSuffix = ".dylib";
}

const filename = `../target/${Deno.args[1]}/${filenamePrefix}${filenameBase}${filenameSuffix}`;

const plugin = Deno.openPlugin(filename);

const { testSync, testAsync } = plugin.ops;

const textDecoder = new TextDecoder();

function testSync() {
  const response = testSync.dispatch(
    new Uint8Array([116, 101, 115, 116]),
    new Uint8Array([116, 101, 115, 116])
  );

  console.log(`Native Binding Sync Response: ${textDecoder.decode(response)}`);
}

testAsync.setAsyncHandler(response => {
  console.log(`Native Binding Async Response: ${textDecoder.decode(response)}`);
});

function testAsync() {
  const response = testAsync.dispatch(
    new Uint8Array([116, 101, 115, 116]),
    new Uint8Array([116, 101, 115, 116])
  );

  if (response != null || response != undefined) {
    throw new Error("Expected null response!");
  }
}

testSync();
testAsync();
