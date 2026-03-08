import { showHUD, showToast, Toast } from "@raycast/api";
import { runTailshare } from "./utils";

export default async function Command() {
  try {
    const output = runTailshare(["get"]);
    await showHUD(output);
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    await showToast({ style: Toast.Style.Failure, title: "Get Failed", message: msg });
  }
}
