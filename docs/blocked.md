# Blocked Work

B02 native macOS Peek menu validation requires an unlocked desktop session.
The current session reported `CGSSessionScreenIsLocked = 1`, and `loginwindow`
(PID 637) remained the foreground application when the test-owned VS Code
process requested activation. The driver refuses to send native menu events
without verified test-process focus. The user has been asked to unlock the
machine.

Provider, keyboard and modifier-click checks can continue through the isolated
workbench driver. The six Peek follow/dismiss/unknown Input/Render obligations
remain unverified; scoped proof collection cannot close B02. After unlocking,
run the complete local input suite and collect fresh installed evidence before
strict B02 acceptance. This is a local execution prerequisite, not deferred
B16 scope.
