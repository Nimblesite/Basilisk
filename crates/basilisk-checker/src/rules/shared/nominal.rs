//! Implements [TYPEINF-SUBTYPING-NOMINAL]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-NOMINAL
//!
//! The module's NOMINAL hierarchy: "is this class a subclass of that one?",
//! answered from resolved class identity.
//!
//! This is the rebuilt replacement for the deleted `crate::subtyping`, whose
//! whole hierarchy was keyed on strings — `is_subtype(&str, &str)` over class
//! names harvested from rendered annotations, with `"object"` matched as a
//! literal and enum membership settled by `strip_prefix`. Every rule that took
//! it inherited a verdict about spelling.
//!
//! One thing here is right, and one is still wrong. Both are stated plainly
//! because a comment that oversells this module would hide the defect it is
//! standing on.
//!
//! **Right:** the edges come from [`basilisk_resolver::ClassGraph`], built from
//! base expressions resolved through the module's binding table and keyed on
//! definition site. Two classes spelled alike are two nodes; one class reached
//! under three names is one node.
//!
//! **Still wrong:** the leaves arrive as `InferredType::Named(String)` — a
//! RENDERING, the [TYPEINF-LEGACY] boundary — and this module recovers an
//! identity from it by handing the string back to `ruff_python_parser` and
//! resolving the reconstructed expression. THAT IS STILL A VERDICT DERIVED
//! FROM TEXT. Re-parsing is better than splitting at `[`, and it is not a fix:
//!
//! * the reconstructed node has no offset in the real file, so resolution
//!   cannot be positional and falls back to the module's FINAL namespace — a
//!   leaf from an annotation written before a rebinding resolves to the class
//!   in force after it;
//! * the enclosing scope is gone, so a class-local or function-local binding
//!   that governed the original expression is invisible;
//! * a leaf whose rendering does not round-trip to the expression it came from
//!   resolves to something else, or to nothing.
//!
//! The fix is not in this file: `InferredType`'s nominal leaf must carry the
//! definition site it was resolved to, taken from the original AST node at the
//! moment the annotation cascade resolved it. Until it does, every answer
//! below is only as good as the rendering it was handed. Do not describe this
//! module as text-free, and do not add another caller that reaches it with a
//! `String` where an `Expr` was available.
//!
//! Every query is three-valued at heart: a leaf this module cannot resolve to
//! a class of its own yields no relation, and callers must treat that as "no
//! evidence" rather than "not a subclass" ([CHKARCH-CONFORMANCE-MODE]).

use basilisk_resolver::{BindingTable, ClassGraph, ClassInfo, ResolvedModule, Span};

/// A module's class hierarchy plus the bindings that name it.
#[derive(Debug)]
pub(crate) struct NominalHierarchy<'m> {
    /// Definition-site-keyed class graph over the module's classes.
    graph: ClassGraph<'m>,
    /// The module's binding table — the only path from a name to a class.
    ///
    /// ORPHANED BY A DELETION, NOT UNUSED. Both readers — `definition_site`
    /// and `is_declared_member` — reached it by re-parsing a rendering and
    /// were deleted for it. The table stays because it is the input the
    /// rebuild takes: an `Expr` at its own offset resolved through here.
    #[expect(
        dead_code,
        reason = "both readers were deleted for reaching this through a re-parsed rendering; \
                  the table is the input the rebuild resolves against"
    )]
    bindings: &'m BindingTable,
}

impl<'m> NominalHierarchy<'m> {
    /// Build the hierarchy for a module.
    pub(crate) fn build(module: &'m ResolvedModule) -> Self {
        Self {
            graph: ClassGraph::new(&module.classes),
            bindings: &module.bindings,
        }
    }

    /// The class a rendered type leaf denotes in this module, if any.
    ///
    /// `Sub`, `Sub[int]`, and `Alias` (bound by `Alias = Sub`) all reach
    /// `Sub`'s definition; a name bound to anything else — an import, a
    /// `TypeVar`, a builtin — reaches nothing.
    pub(crate) fn class_of(&self, rendering: &str) -> Option<&'m ClassInfo> {
        let site = self.definition_site(rendering)?;
        self.graph.at(site)
    }

    // ######################################################################
    // # DELETED BODY — `definition_site`. DO NOT RESTORE IT.               #
    // #                                                                     #
    // #   let parsed = ruff_python_parser::parse_expression(rendering)?;    #
    // #   self.bindings.deferred_local_class(parsed.expr()).map(Span::from) #
    // #                                                                     #
    // # A class identity recovered by HANDING A STRING BACK TO THE PARSER.  #
    // # Re-parsing is better than splitting at `[` and it is not a fix —    #
    // # the reconstructed node has no offset in the real file, so:          #
    // #                                                                     #
    // #   * resolution cannot be positional and falls back to the module's  #
    // #     FINAL namespace, so a leaf from an annotation written before a  #
    // #     rebinding resolves to the class in force after it;              #
    // #   * the enclosing scope is gone, so a class-local or function-local #
    // #     binding that governed the original expression is invisible;     #
    // #   * a rendering that does not round-trip to the expression it came  #
    // #     from resolves to something else, or to nothing.                 #
    // #                                                                     #
    // # The fix is not in this file. `InferredType`'s nominal leaf must     #
    // # carry the definition site it was resolved to, taken from the        #
    // # original AST node at the moment the annotation cascade resolved it  #
    // # ([TYPEINF-LEGACY]). `class_of` above is kept as the map of what     #
    // # reads this.                                                         #
    // #                                                                     #
    // # Pinned by: tests/nominal_leaf_identity_tests.rs                     #
    // ######################################################################

    /// DELETED — panics; see the banner above.
    fn definition_site(&self, _rendering: &str) -> Option<Span> {
        panic!(
            "basilisk-checker: `NominalHierarchy::definition_site` was DELETED because it \
             recovered a CLASS IDENTITY by re-parsing a rendered type leaf and resolving \
             the reconstructed expression against the module's final namespace — no \
             offset, no scope, no round-trip guarantee. It panics because the real \
             implementation DOES NOT EXIST YET: `InferredType`'s nominal leaf must carry \
             the definition site it resolved to. Do not restore the parse-back and do not \
             return `None` in its place."
        )
    }

    /// Is the class `sub` denotes a subclass of the class `sup` denotes?
    ///
    /// `None` when either leaf does not name a class this module defines, and
    /// also when the answer would be NEGATIVE but `sub`'s ancestry contains an
    /// edge this module cannot follow. A base imported from elsewhere may
    /// itself derive from `sup`, so "not found in the chain I could walk" is
    /// not the same as "not a subclass" ([CHKARCH-CONFORMANCE-MODE]). A
    /// POSITIVE answer needs no such caution: finding `sup` in the chain
    /// proves the relation whatever else is unknown.
    pub(crate) fn is_subclass(&self, sub: &str, sup: &str) -> Option<bool> {
        let sub_class = self.class_of(sub)?;
        let sup_site = self.class_of(sup)?.name_span;
        let ancestry = self.graph.ancestry(sub_class);
        if ancestry
            .classes
            .iter()
            .any(|ancestor| ancestor.name_span == sup_site)
        {
            return Some(true);
        }
        ancestry.complete.then_some(false)
    }

    /// Is the rendered leaf `member` an ENUM MEMBER of the class `owner`
    /// denotes — `Colour.OCHRE` against `Colour`?
    ///
    /// This asks the one question for which `C.X` is a value OF TYPE `C`.
    /// [PEP 435](https://peps.python.org/pep-0435/) makes each member of an
    /// enumeration an instance of the enumeration itself; no other class works
    /// that way, so `class Ledger: rate = 1` gives `Ledger.rate` the type of
    /// `1`, not `Ledger`.
    ///
    /// The deleted layer answered with `sub.strip_prefix(sup)`, which accepted
    /// any class whose name merely began with the target's and rejected a
    /// genuine member reached under an alias. An earlier repair resolved the
    /// qualifier properly but still accepted ANY declared attribute of ANY
    /// class, which made every class-level constant an inhabitant of its own
    /// class. Enum-ness comes from `ClassInfo::is_enum`, which the resolver
    /// closes over definition-site-keyed base edges, so an enum reached
    /// through an alias is still an enum here.
    ///
    // ######################################################################
    // # DELETED BODY — `is_declared_member`. DO NOT RESTORE IT.            #
    // #                                                                     #
    // #   let parsed = ruff_python_parser::parse_expression(member.trim())?;#
    // #   let Expr::Attribute(attribute) = parsed.expr() else { .. };       #
    // #   self.bindings.deferred_local_class(&attribute.value)              #
    // #                                                                     #
    // # THE SAME DEFECT AS `definition_site` ABOVE, IN A SECOND PLACE. An   #
    // # enum-membership verdict recovered by HANDING A STRING BACK TO THE   #
    // # PARSER and resolving the reconstructed node, so:                    #
    // #                                                                     #
    // #   * the qualifier resolved against the module's FINAL namespace     #
    // #     rather than the offset the leaf came from — `Colour.OCHRE`      #
    // #     written before `Colour` is rebound resolves to the class in     #
    // #     force after it;                                                 #
    // #   * a leaf rendered inside a function or class body lost its        #
    // #     enclosing scope entirely;                                       #
    // #   * a rendering that does not round-trip — anything the renderer    #
    // #     abbreviated, requoted, or normalised — resolved to something    #
    // #     else, or to nothing, and `Some(false)` was returned as a        #
    // #     POSITIVE NEGATIVE from that.                                    #
    // #                                                                     #
    // # The fix is the same fix: `InferredType`'s nominal leaf must carry   #
    // # the definition site AND the member identity it resolved to, taken   #
    // # from the original AST node ([TYPEINF-LEGACY]). Its call site in     #
    // # `shared/judge.rs::nominal_subclass_assignable` is kept as the map   #
    // # of what has to be rebuilt.                                          #
    // #                                                                     #
    // # Pinned by: tests/nominal_leaf_identity_tests.rs                     #
    // ######################################################################

    /// DELETED — panics; see the banner above.
    pub(crate) fn is_declared_member(&self, _member: &str, _owner: &str) -> Option<bool> {
        panic!(
            "basilisk-checker: `NominalHierarchy::is_declared_member` was DELETED because it \
             decided ENUM MEMBERSHIP by re-parsing a rendered type leaf and resolving the \
             reconstructed attribute expression against the module's final namespace — no \
             offset, no scope, no round-trip guarantee — and returned `Some(false)` from \
             it. It panics because the real implementation DOES NOT EXIST YET: \
             `InferredType`'s nominal leaf must carry the definition site it resolved to. \
             Do not restore the parse-back and do not return `None` in its place."
        )
    }
}
