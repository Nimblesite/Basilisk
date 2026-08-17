// Implements [VSIX-CONFIGURATION-EDITOR-HOST].
/** Complete static webview runtime assembled from focused fragments. */

import { CONFIGURATION_EDITOR_SCRIPT_CACHE } from "./configuration-editor-script-cache";
import { CONFIGURATION_EDITOR_SCRIPT_CORE } from "./configuration-editor-script-core";
import { CONFIGURATION_EDITOR_SCRIPT_EVENTS } from "./configuration-editor-script-events";
import { CONFIGURATION_EDITOR_SCRIPT_RENDER } from "./configuration-editor-script-render";
import { CONFIGURATION_EDITOR_SCRIPT_TYPESHED } from "./configuration-editor-script-typeshed";

export const CONFIGURATION_EDITOR_SCRIPT = [
  CONFIGURATION_EDITOR_SCRIPT_CORE,
  CONFIGURATION_EDITOR_SCRIPT_RENDER,
  CONFIGURATION_EDITOR_SCRIPT_TYPESHED,
  CONFIGURATION_EDITOR_SCRIPT_CACHE,
  CONFIGURATION_EDITOR_SCRIPT_EVENTS,
].join("\n");
