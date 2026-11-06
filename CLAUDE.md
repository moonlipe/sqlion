<!-- rtk-instructions v2 -->
# Command output

Command output here is condensed to save tokens, keeping every signal and
dropping costly noise. Treat it as the complete result: run commands
normally, and batch related commands into one call to avoid extra turns.
Truncated results state their recovery path in their own output. Re-run a
command as `rtk bypass <cmd>` only when its result is unusable: empty when
output was clearly expected, contradicting its exit code, or garbled.

Delegation commands (they hand work to a cheaper model; the files never enter your context):
- `rtk bulk-read --question "..." <paths...>`
- `rtk code-write --spec "..." --reference <file> --target <file>`
<!-- /rtk-instructions -->