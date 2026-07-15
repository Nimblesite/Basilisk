# typeDiagram model - https://typediagram.dev
# The Basilisk configuration model ([CHKARCH-CONFIG-MODEL]).
#
# This is the WHOLE config model: a flat list of explicit rule entries.
# A rule with no entry does not run. Folder configs override ancestors per
# rule, nearest entry wins ([CHKARCH-CONFIG-DISCOVERY]). Editor/LSP wire types
# live in models/configuration_editor.td and MUST NOT leak into this file.
#
# Render the documentation diagram:
#   typediagram models/configuration.td > docs/models/configuration.svg

alias RuleCode = String

# The four values an entry can state. There are no default severities and no
# inherited state; a rule without an entry is disabled.
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

# One folder's [tool.basilisk] table.
type RulesConfig {
  entries: List<RuleEntry>
}
