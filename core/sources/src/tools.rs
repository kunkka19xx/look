//! Which command a tool action runs for a row: the block's own verb, else the
//! user's declared tool, else the platform default.
//!
//! Here because this is the only crate that sees both: `look-sources` already
//! depends on `look-tools`, so the dependency cannot run the other way, and a
//! shell holding the rule would mean every shell reimplementing it.

use look_tools::{Action, Launch, Tools, Unavailable};

use crate::def::Block;
use crate::run::{RowContext, expand};

/// What `action` does to `row`, or `None` when the action is unknown or the row
/// has no path, exactly as `look_tools::resolve` answers for a row with no
/// block.
///
/// A block's verb wins for its own rows, expanded like every other command it
/// declares. Declaring nothing changes nothing.
pub fn resolve_for_row(
    action: &str,
    path: &str,
    is_dir: bool,
    tools: &Tools,
    block: Option<&Block>,
    row: &RowContext,
) -> Option<Result<Launch, Unavailable>> {
    let declared = Action::from_id(action)
        .and_then(|action| block.and_then(|block| block.verbs.for_action(action)));

    match declared {
        // The guard `look_tools::resolve` applies, before taking its own path.
        Some(_) if path.trim().is_empty() => None,
        Some(command) => Some(Ok(Launch::Shell {
            // The block, not a tool: no tool was consulted.
            tool: block.map(|block| block.name.clone()).unwrap_or_default(),
            command: expand(command, row),
        })),
        None => look_tools::resolve(action, path, is_dir, tools),
    }
}

/// Whether `block` takes `action` for its own rows, which is what a label needs
/// to know: the same lookup `resolve_for_row` makes.
pub fn block_declares(block: Option<&Block>, action: &str) -> bool {
    Action::from_id(action)
        .and_then(|action| block.and_then(|block| block.verbs.for_action(action)))
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::parse_file;
    #[cfg(windows)]
    use look_tools::cmd_quote as quote;
    #[cfg(not(windows))]
    use look_tools::shell_quote as quote;

    fn block(contents: &str) -> Block {
        parse_file(contents)
            .expect("valid file")
            .blocks
            .into_iter()
            .next()
            .expect("one block")
    }

    fn row() -> RowContext {
        RowContext {
            id: "main".into(),
            title: "main".into(),
            path: "/dev/look".into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_blocks_verb_wins_for_its_own_rows_and_expands_like_any_other() {
        let block =
            block("[projects]\ndir = \"~/dev\"\nterminal = \"tmux new -As {title} -c {path}\"\n");
        let launch = resolve_for_row(
            "terminal",
            "/dev/look",
            true,
            &Tools::default(),
            Some(&block),
            &row(),
        )
        .expect("known action")
        .expect("declared");

        match launch {
            Launch::Shell { tool, command } => {
                assert_eq!(
                    command,
                    format!("tmux new -As {} -c {}", quote("main"), quote("/dev/look"))
                );
                assert_eq!(tool, "projects", "the block is what decided");
            }
            other => panic!("expected shell, got {other:?}"),
        }
    }

    #[test]
    fn a_verb_the_block_does_not_declare_falls_through_untouched() {
        // Declaring `terminal` must not change what Cmd+E does.
        let block = block("[projects]\ndir = \"~/dev\"\nterminal = \"tmux new\"\n");
        let fell_through = resolve_for_row(
            "edit",
            "/dev/look/main.rs",
            false,
            &Tools::default(),
            Some(&block),
            &row(),
        );
        assert_eq!(
            format!("{fell_through:?}"),
            format!(
                "{:?}",
                look_tools::resolve("edit", "/dev/look/main.rs", false, &Tools::default())
            )
        );
        assert!(!block_declares(Some(&block), "edit"));
        assert!(block_declares(Some(&block), "terminal"));
    }

    #[test]
    fn a_row_from_no_block_resolves_exactly_as_before() {
        let path = "/dev/look/main.rs";
        assert_eq!(
            format!(
                "{:?}",
                resolve_for_row("edit", path, false, &Tools::default(), None, &row())
            ),
            format!(
                "{:?}",
                look_tools::resolve("edit", path, false, &Tools::default())
            )
        );
    }

    #[test]
    fn an_unknown_action_or_an_empty_path_answers_nothing() {
        let block = block("[projects]\ndir = \"~/dev\"\nterminal = \"tmux new\"\n");
        assert!(
            resolve_for_row(
                "nope",
                "/dev/look",
                true,
                &Tools::default(),
                Some(&block),
                &row()
            )
            .is_none()
        );
        assert!(
            resolve_for_row(
                "terminal",
                "",
                true,
                &Tools::default(),
                Some(&block),
                &row()
            )
            .is_none()
        );
    }
}
