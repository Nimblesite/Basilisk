// Implements [WEBSITE-ERROR-PAGES]: headline counts for the rules overview, kept
// in sync with the checker source so the prose can never drift from the table.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const rules = JSON.parse(readFileSync(join(here, "rules.json"), "utf8"));

const errors = rules.filter((rule) => rule.severity === "error").length;

export default { total: rules.length, errors, warnings: rules.length - errors };
