//! Turning a declaration off without deleting it.
//!
//! `enabled` is a key the loader already honours, so off is a state the file
//! holds rather than one the app remembers beside it. The screen offering the
//! switch also offers Edit and Reveal, so a block that produces nothing has to
//! say so in the file somebody then opens.
//!
//! Written here for the reason `fill` lives next door: the shell never builds
//! TOML, and a second place that edits a declaration is how two of them learn
//! to disagree about what one looks like.
//!
//! Every block in the file at once, because the file is the unit a user
//! installs, deletes, and sees one row for.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::setup::without_setup;

/// Each block with the span of its body, and each of its keys with the span of
/// its value. Taken from the document with `[[setup]]` blanked out, so the
/// spans still address the original text.
type Blocks = BTreeMap<String, toml::Spanned<BTreeMap<String, toml::Spanned<toml::Value>>>>;

/// The key this writes. Reserved to the loader, not to a block's author.
const KEY_ENABLED: &str = "enabled";

/// `contents` with every block's `enabled` set to `on`.
///
/// Rewrites the value where the key is declared and inserts the line ahead of
/// the block's first key where it is not. By span, like `fill`: a declaration
/// is mostly prose in the examples, and re-serialising the document would
/// throw every comment in it away.
pub fn set_enabled(contents: &str, on: bool) -> Result<String, String> {
    let blocks: Blocks = toml::from_str(&without_setup(contents)).map_err(|err| err.to_string())?;
    if blocks.is_empty() {
        return Err("no block to turn off".into());
    }
    let value = if on { "true" } else { "false" };

    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for block in blocks.values() {
        match block.get_ref().get(KEY_ENABLED) {
            Some(declared) => edits.push((declared.span(), value.to_string())),
            None => {
                let at = insertion(contents, block);
                edits.push((at..at, format!("{KEY_ENABLED} = {value}\n")));
            }
        }
    }

    // Back to front, so an earlier edit cannot move a span taken later.
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut out = contents.to_string();
    for (span, text) in edits {
        out.replace_range(span, &text);
    }
    Ok(out)
}

/// Where a line declaring the key goes in a block that has none: on its own
/// line ahead of the block's first key.
///
/// Ahead of the key rather than straight under the header because a comment
/// written under the header belongs to the key below it, and a line pushed
/// between the two reads as an answer to that comment.
fn insertion(
    contents: &str,
    block: &toml::Spanned<BTreeMap<String, toml::Spanned<toml::Value>>>,
) -> usize {
    let header = block.span().start;
    match block.get_ref().values().map(|key| key.span().start).min() {
        Some(first) => contents[..first].rfind('\n').map_or(0, |line| line + 1),
        // A block with no keys at all: nothing to go ahead of, so straight
        // after its header.
        None => contents[header..]
            .find('\n')
            .map_or(contents.len(), |line| header + line + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_key_into_every_block_that_lacks_it() {
        let off = set_enabled(
            "[notes]\ndir = \"~/notes\"\n\n[deploy]\ndo = [\"make\"]\n",
            false,
        )
        .expect("blocks");
        assert_eq!(
            off,
            "[notes]\nenabled = false\ndir = \"~/notes\"\n\n[deploy]\nenabled = false\ndo = [\"make\"]\n"
        );
        assert_eq!(
            crate::parse_file(&off)
                .expect("still parses")
                .blocks
                .iter()
                .filter(|block| block.enabled)
                .count(),
            0
        );
    }

    #[test]
    fn rewrites_the_value_a_block_already_declares() {
        let on = set_enabled(
            "[notes]\nenabled = false # off for now\ndir = \"~/n\"\n",
            true,
        )
        .expect("blocks");
        assert_eq!(
            on, "[notes]\nenabled = true # off for now\ndir = \"~/n\"\n",
            "the value only, not the comment after it"
        );
    }

    #[test]
    fn keeps_the_comments_and_the_setup_fields() {
        let file = "# what this is\n[[setup]]\nblock = \"notes\"\nkey = \"dir\"\nlabel = \"Where\"\n\n[notes]\n# the folder\ndir = \"~/notes\"\n";
        let off = set_enabled(file, false).expect("blocks");
        assert!(off.contains("# what this is"));
        assert!(off.contains("[[setup]]"));
        assert!(
            off.contains("# the folder\nenabled = false\ndir = \"~/notes\""),
            "{off}"
        );
        let (fields, problems) = crate::parse_setup(&off).expect("setup survives");
        assert!(problems.is_empty());
        assert_eq!(fields.len(), 1, "the field still points at its key");
    }

    #[test]
    fn refuses_what_is_not_a_file_of_blocks() {
        assert!(set_enabled("name = \"not a block\"\n", false).is_err());
        assert!(set_enabled("", false).is_err(), "nothing to turn off");
    }
}
