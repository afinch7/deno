import { testOp } from "../target/release/libtest_binding_plugin.so";

console.log(testOp({ name: "test" }));
