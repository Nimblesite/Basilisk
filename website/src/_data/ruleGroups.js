// Implements [WEBSITE-ERROR-PAGES]: group the diagnostic codes for the error
// reference directory. Done in data (not a Nunjucks selectattr, which does not
// filter reliably here) so /errors/ lists each group exactly once, in order.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const rules = JSON.parse(readFileSync(join(here, "rules.json"), "utf8"));

const ORDER = ["Missing Annotations", "Type Safety", "Type System", "Warnings"];

export default ORDER.map((group) => ({
  group,
  id: group.toLowerCase().replace(/\s+/g, "-"),
  items: rules.filter((rule) => rule.group === group),
})).filter((group) => group.items.length > 0);
