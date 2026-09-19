# Declaration rename capture slice

B04.02 advances the positive and negative `rename/syntax/S2` service/protocol
partitions. These broad cells remain unreviewed: this slice establishes source
function capture safety, not complete S2 coverage or B04 acceptance.

The shared fixture independently enumerates 16 cross-file scenarios, each run
with LF and CRLF and Chinese/non-BMP text before queried tokens. Rejection cases
cover parameters, ordinary/nested locals, lambda parameters/captures, loop locals
and earlier/later/current parameters in default expressions. Allowed cases cover
uses before local declarations, local initializers, disjoint blocks/functions,
qualified paths, loop iterables and retained import aliases.

Both layers verify the original call's exact declaration target before renaming,
prepareRename eligibility at declaration/import/use sites, and the independently
specified allow/reject result at every edited site. The service applies every
successful plan and compares entire files. The protocol additionally compares
the complete UTF-16 edit set, applies it with an independent edit oracle, and
sends didChange notifications. Both parse the expected changed files and query
the resulting call's exact declaration target. Rejected requests leave the
original ownership intact. Alias spellings remain unchanged.

The initial negative parameter case reproduced a silent change of binding after
rename. Declaration rename now checks bare resolved uses against local visibility;
qualified uses and retained aliases bypass that bare-name collision check. Default
expressions require an additional check against their parent parameters: the
canonical HIR binder declares all parameters before lowering default expressions.
An initial positive assumption for later parameters failed the applied ownership
assertion and was corrected to rejection after inspecting this binding contract.
This change preserves existing binding/runtime behavior rather than redefining
default-parameter scope to match completion's source-order candidate policy.

Outstanding work includes local-target rename capture and disjoint-scope policy,
other declaration/member/schema partitions, lifecycle matrices, installed provider
coverage and UX05/UX06. No new broad syntax cell is marked verified by this slice.
Accepted B00-B03 snapshots remain fixed, with fresh regression evidence from the
current registered profile. macOS requires its own fresh evidence; B16 is deferred.
