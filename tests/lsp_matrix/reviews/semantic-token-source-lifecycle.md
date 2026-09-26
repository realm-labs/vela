# B06 semantic-token source lifecycle review

This child owns the full/delta/range protocol dirty, close_restore, recovery,
unresolved, dynamic, dependency_change and dependency_delete obligations (21
cells). The independent five-file fixture spells every token's text, kind,
complete modifiers and marked UTF-16 range across 27 states. LF and CRLF runs
exercise 54 workspace states and 110 complete document streams, including empty
streams for the deleted dependency. A closed, unimported module deliberately
duplicates the dependency's short names; it cannot supply replacement facts.

| State | Independent expectations |
| --- | --- |
| Disk and overlay | Initial files are actually closed. Only explicit opens create overlays. Dirty importer and owner content wins over watched disk writes, including a malformed disk importer and deletion of an open owner. Every phase checks the complete expected disk/overlay dictionaries, physical files and effective SourceDb text. |
| Close, save and repair | Closing selects the current disk text, including an Any-returning dependency and malformed importer. Saving physically commits the dirty Unicode-prefixed text and sends save plus a watched change, matching the advertised client contract. A later unsaved edit is discarded on close. Full repair restores original data and result IDs. |
| Recovery | A member dot without a name preserves neighboring token facts without inventing a member. This recoverable form has no E_PARSE. A missing function closing brace produces E_PARSE while valid imports, typed members and neighboring calls retain their exact roles. Repaired open text clears E_PARSE. |
| Unresolved | An unknown bare callee is unresolved; known imported owners, members and callable neighbors retain their facts. No symbol is borrowed from an unrelated module. |
| Dynamic return | A known imported grant function changes its return from Cell to Any. Its own source callable facts remain. Members of the returned value lose property/method and source modifiers, while explicitly typed Cell and Reader receivers retain their source facts. An Any parameter never gains member facts. |
| Dependency replacement | Renaming fields, methods, trait methods and variants changes the consumer's complete token stream without editing its text. Existing imported owner types and functions remain source-owned; removed members are lexical and the removed variant is unresolved. Restoring the owner restores all original facts. |
| Dependency deletion | A deleted file remains effective while open. Closing it removes its record and all imported ownership/member facts. Querying the absent dependency returns an empty full stream and empty ranges. Recreating the file restores the original caller facts; duplicate short names in the closed decoy do not stand in for the missing import. |
| Full, delta and range | Every state checks the independently authored complete stream, repeat stability and parity with a fresh server reading the actual disk plus explicit overlays. Deltas from the preceding and original IDs apply exactly to current data. Current-ID deltas have no edits. Each token range, each line range and empty token-end/document-start ranges are checked, with token ranges repeated and compared to fresh results. Queries cannot parse, rebuild project/HIR or advance the generation. |

The fixture uses encoded Chinese, space and percent paths, non-BMP prefixes and
both newline styles. Assertions run through the production protocol dispatcher;
expected classifications never come from a provider. Source text is queried as
metadata only, without VM or live host execution.

Schema missing/stale state cells and editor interaction/render cells remain
separate. Accepted B00-B05 snapshots are preserved; Windows evidence cannot
close macOS gates. B16 stays deferred.
