import { installBrowserFocusGuardRegistry } from "./focus-guard.js";

if (!installBrowserFocusGuardRegistry(window, document)) {
  console.error("bekoedit: incompatible source focus guard registry");
}
