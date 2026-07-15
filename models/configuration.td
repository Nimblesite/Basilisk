# typeDiagram model - https://typediagram.dev
# The Basilisk configuration model ([CHKARCH-CONFIG-MODEL]).
#
# This is the WHOLE config model. One universe of rules, partitioned once by
# provenance tag: `pep`-tagged rules belong to `basilisk check` and ALWAYS run
# (config grades them, never disables them); every other rule belongs to
# `basilisk analyze` and runs only when config resolves it to a non-disabled
# severity ([CHKARCH-COMMANDS]).
#
# Resolution, per rule, per checked file: walk from the file's folder upward;
# the NEAREST [tool.basilisk] table that decides the rule wins. Within one
# table a per-rule entry beats tag entries; among matching tag entries the
# strictest severity wins (error > warning > info > disabled). Nothing
# anywhere: pep rules run at error, everything else is disabled.
#
# Editor/LSP wire types live in models/configuration_editor.td and MUST NOT
# leak into this file.
#
# Render the documentation diagram:
#   typediagram models/configuration.td > docs/models/configuration.svg

alias RuleCode = String
alias RuleTag = String

# The four values an entry can state. Disabled can never apply to a
# pep-tagged rule: a config that resolves one to disabled is invalid.
union RuleSeverity {
  Error
  Warning
  Info
  Disabled
}

# One line in [tool.basilisk.rules]: "<code>" = "<severity>".
type RuleEntry {
  code: RuleCode
  severity: RuleSeverity
}

# One line in [tool.basilisk.rule-tags]: "<tag>" = "<severity>".
# An explicit, written group entry - e.g. "basilisk" = "error" turns every
# house rule on. Tags are never implicit switches; a tag entry is config.
type TagEntry {
  tag: RuleTag
  severity: RuleSeverity
}

# One folder's [tool.basilisk] table. A missing table and an empty table
# behave identically (pep rules at error, nothing else runs); only the LSP's
# one-time seed distinguishes them ([LSPARCH-CONFIG-SEEDING]).
type RulesConfig {
  rules: List<RuleEntry>
  ruleTags: List<TagEntry>
}
