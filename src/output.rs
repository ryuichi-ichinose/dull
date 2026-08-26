use indicatif::MultiProgress;
use std::io::IsTerminal;

/// `MultiProgress::println` silently drops output when stderr isn't a
/// terminal (indicatif hides ticks and one-off messages alike), which would
/// swallow real error text in a CI log. Fall back to eprintln! there.
pub fn multi_println(multi: &MultiProgress, msg: impl std::fmt::Display) {
    let msg = msg.to_string();
    if std::io::stderr().is_terminal() {
        let _ = multi.println(msg);
    } else {
        eprintln!("{msg}");
    }
}
