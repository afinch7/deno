import { core } from "./core";
import { sendSync } from "./dispatch";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";

export const nativeBindings = {
  get opIds(): OpIdsRoot {
    return core.opIds;
  },
  sendSync(opId: number, data: Uint8Array): Uint8Array | null {
    const builder = flatbuffers.createBuilder();
    const inner = msg.CustomOp.createCustomOp(builder, opId);
    const baseRes = sendSync(builder, msg.Any.CustomOp, inner, data);
    assert(baseRes != null);
    assert(
      msg.Any.CustomOpRes === baseRes!.innerType(),
      `base.innerType() unexpectedly is ${baseRes!.innerType()}`
    );
    const res = new msg.CustomOpRes();
    assert(baseRes!.inner(res) != null);

    return res.dataArray();
  }
};
