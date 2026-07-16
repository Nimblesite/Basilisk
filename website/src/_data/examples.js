// Implements [WEBSITE-ERROR-PAGES-EXAMPLES]: map each diagnostic code to the
// worked-example screenshot that demonstrates it, so /errors/<code>/ can embed
// the real `basilisk check` output. See docs/specs/WEBSITE-ERROR-PAGES-SPEC.md.
//
// The screenshot manifest is the single source of truth: each rule shot records
// the exact code it triggers in `expect` (e.g. e0011 → BSK-0014), so we key off
// that rather than the filename to stay correct even where they differ.
import { SHOTS } from "../../screenshots/shots.mjs";

const RULE_SHOT = /^e\d+$/;
const RULE_CODE = /^BSK-\d{4}$/;

export default Object.fromEntries(
  SHOTS.filter((shot) => RULE_SHOT.test(shot.name) && RULE_CODE.test(shot.expect)).map(
    (shot) => [shot.expect, shot.name],
  ),
);
