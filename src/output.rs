use indicatif::MultiProgress;
use std::io::IsTerminal;

/// Prints a message alongside active progress bars.
///
/// `MultiProgress::println` silently drops its output when stderr isn't a
/// terminal (indicatif hides the whole draw target, ticks and one-off
/// messages alike, to avoid spamming redirected output with redraws). That's
/// correct for the ticks but wrong for messages like build errors, which
/// still need to reach a CI log. Route around it in that case instead of
/// losing the message.
pub fn multi_println(multi: &MultiProgress, msg: impl std::fmt::Display) {
    let msg = msg.to_string();
    if std::io::stderr().is_terminal() {
        let _ = multi.println(msg);
    } else {
        eprintln!("{msg}");
    }
}
