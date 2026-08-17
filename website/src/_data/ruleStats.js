// Implements [WEBSITE-ERROR-PAGES]: headline counts for the rules overview, kept
// in sync with the checker source so the prose can never drift from the table.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const rules = JSON.parse(readFileSync(join(here, "rules.json"), "utf8"));

const errors = rules.filter((rule) => rule.severity === "error").length;
// Provenance split — the axis that actually matters on the homepage: the PEP
// typing-spec rules the conformance suite grades (on by default) are counted
// SEPARATELY from Basilisk's opt-in house-style rules (off by default). Never
// lump the two into one headline number. Provenance is the checker's own
// `opt_in_spec` signal, threaded through rules.json by gen_rules_reference.py.
const optIn = rules.filter((rule) => rule.provenance === "basilisk").length;
const pep = rules.length - optIn;
// The opt-in rules that enforce *real* typing beyond the spec — the ones the
// checker tags `strictness` (require an annotation on every parameter, return,
// variable, vararg, and attribute; require @override; reject implicit `Any`;
// annotate lambdas). Counted from the checker's own tags, so it can't drift.
const strictness = rules.filter((rule) =>
  (rule.tags || []).includes("strictness"),
).length;

export default {
  total: rules.length,
  errors,
  warnings: rules.length - errors,
  pep,
  optIn,
  strictness,
};
