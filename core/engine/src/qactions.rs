//! Which control a result carries: the platform binding table, or a
//! declaration. Here rather than in `look_qactions` because only the engine
//! knows both - the catalog stays a leaf with no idea sources exist.

use look_indexing::CandidateIdKind;
use look_qactions::ActionDescriptor;

/// Every control the result carries.
pub fn descriptors_for(result_id: &str, kind: &str) -> Vec<ActionDescriptor> {
    let native = look_qactions::descriptors_for(result_id, kind);
    if !native.is_empty() {
        return native;
    }
    declared_control(result_id)
        .map(|control| descriptor_of(&control))
        .into_iter()
        .collect()
}

/// A declaration may only offer a control it SUPPLIES, never a system one: a
/// block that runs `true` could otherwise show the machine's Bluetooth, and a
/// source copied off the internet could put Shut Down under a row that looks
/// like a note.
fn declaration_may_offer(control: &str) -> bool {
    look_qactions::descriptor(control).is_none()
}

/// Why a declared control shows nothing, or `None` when it does show.
pub fn declared_control_problem(control: &str) -> Option<String> {
    (!declaration_may_offer(control)).then(|| {
        format!(
            "cannot offer the system control '{control}'; a declaration may only offer a control it supplies"
        )
    })
}

/// The id a shell sends back when the control is pressed. The block is in it
/// because the press has to reach whoever produced the row, and the control is
/// in it because one block may carry more than one in time.
pub fn control_action_id(block_id: &str, control_id: &str) -> String {
    format!("{DECLARED}{block_id}{SEP}{control_id}")
}

/// A drawing names a control as `<block>#<control>`; the id a shell sends back
/// is the same with the marker on the front.
pub fn control_action_id_from_ref(reference: &str) -> Option<String> {
    let (block, control) = reference.rsplit_once(SEP)?;
    Some(control_action_id(block, control))
}

/// The block and control halves of one, or None when it is not one.
pub fn control_parts(action_id: &str) -> Option<(&str, &str)> {
    action_id.strip_prefix(DECLARED)?.rsplit_once(SEP)
}

/// Marks an id as supplied rather than native, so a shell can tell at a glance
/// which adapter runs it.
const DECLARED: &str = "declared:";
/// Last, not first: a block id holds dots, and a control id may hold anything
/// but this.
const SEP: char = '#';

fn descriptor_of(control: &look_sources::Control) -> ActionDescriptor {
    ActionDescriptor {
        action_id: control.id.clone(),
        title: control.title.clone(),
        control: match control.kind {
            look_sources::ControlKind::Toggle => look_qactions::ControlKind::Toggle,
            look_sources::ControlKind::Button => look_qactions::ControlKind::Button,
        },
        on_label: control.on.clone(),
        off_label: control.off.clone(),
        // Values come from the connector with the state, not from a native
        // adapter resolving a key.
        info: Vec::new(),
    }
}

/// The control the producing block declared, with its id already namespaced by
/// that block. Reads the sources directory: a per-selection cost, never a
/// per-keystroke one.
fn declared_control(result_id: &str) -> Option<look_sources::Control> {
    let block_id = CandidateIdKind::source_id_of(result_id)?;
    let mut control = crate::sources::find_block(block_id)?.control?;
    if !declaration_may_offer(&control.id) {
        return None;
    }
    control.id = control_action_id(block_id, &control.id);
    Some(control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_no_one_named_carries_nothing() {
        assert!(descriptors_for("file:/tmp/x", "file").is_empty());
    }

    #[test]
    fn a_declaration_may_not_offer_a_system_control() {
        for control in [
            look_qactions::action_id::BLUETOOTH,
            look_qactions::action_id::WIFI,
            look_qactions::action_id::SHUTDOWN,
            look_qactions::action_id::RESTART,
        ] {
            assert!(
                !declaration_may_offer(control),
                "{control} must stay bound by the platform table"
            );
        }
    }

    #[test]
    fn a_name_no_catalog_knows_is_not_a_system_control() {
        // Refused and absent are different reasons; only the refusal is a rule.
        assert!(declaration_may_offer("myapp.lamp"));
        assert!(look_qactions::descriptor("myapp.lamp").is_none());
    }

    #[test]
    fn only_a_borrowed_control_is_a_problem() {
        let refused = declared_control_problem(look_qactions::action_id::BLUETOOTH).unwrap();
        assert!(refused.contains("may only offer a control it supplies"));
        // A name nobody else uses is exactly what a declaration should supply.
        assert_eq!(declared_control_problem("lamp"), None);
    }

    #[test]
    fn an_action_id_carries_the_block_that_must_hear_the_press() {
        let id = control_action_id("myapp.records", "sync");
        assert_eq!(control_parts(&id), Some(("myapp.records", "sync")));
        // What a drawing writes resolves to the same id.
        assert_eq!(
            control_action_id_from_ref("myapp.records#sync"),
            Some(id.clone())
        );
        assert_eq!(control_action_id_from_ref("nohash"), None);
        // A block id holds dots, so the split cannot be on the first one.
        assert_eq!(control_parts("declared:a.b.c#x"), Some(("a.b.c", "x")));
        assert_eq!(control_parts("bluetooth"), None);
    }
}
