//! Which part of an existing deploy log a new deploy keeps.

use crate::log_stamp::split_stamp;

use super::DEPLOY_MARK;

/// Byte offsets of the lines that start a deploy.
fn deploy_starts(text: &str) -> Vec<usize> {
    let mut offset = 0;
    let mut starts = Vec::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if split_stamp(bare).is_some_and(|(_, rest)| rest == DEPLOY_MARK) {
            starts.push(offset);
        }
        offset += line.len();
    }
    starts
}

/// The tail of `text` holding at most its last `keep` deploys and at most
/// `max_bytes`, always starting at a deploy's first line. Text before the
/// first deploy line, or a single deploy larger than `max_bytes`, is
/// dropped whole.
pub(super) fn recent_deploys(text: &str, keep: usize, max_bytes: usize) -> &str {
    let starts = deploy_starts(text);
    let first = starts.len().saturating_sub(keep);
    starts[first..]
        .iter()
        .find(|start| text.len() - **start <= max_bytes)
        .map_or("", |start| &text[*start..])
}
