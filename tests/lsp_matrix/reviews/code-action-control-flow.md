# S6 pattern and control-flow code actions

B05.21 adds LF/CRLF service and protocol fixtures for exhaustive and missing
`Option`/`Result` match arms, guarded arms, wildcard matches, dynamic
scrutinees, `for`/`if`/`break`/`continue`, inline matches and incomplete syntax.
Three distinct warnings each yield one exact action. `Option::None` is a unit
pattern; missing `Option::Some` and `Result::Ok` arms bind their payload with
`_`. Each action is applied to the whole source and clears only its warning;
applying all three clears every warning and action. Fresh instances agree,
and closing a dirty protocol document restores the disk warnings and actions.

A second fixture loads a real schema enum with unit, two-field tuple and
record variants. The action inserts `State::Named { value: _ }` and
`State::Pair(_, _)` arms, preserving both variant shape and arm indentation.
Applying it clears the warning, and valid existing patterns remain action-free.
Unknown and irregular schema shapes are not turned into guessed edits.

The tests caught two defects in the previous action generator: it inserted LF
into CRLF documents and lost the closing-block indentation on the second of
multiple generated arms. The action now uses the match block's own line ending
and restores indentation before each additional arm. Protocol tests assert
complete versioned and unversioned edits, UTF-16 request ranges after Chinese
and non-BMP characters, and encoded workspace URIs.
