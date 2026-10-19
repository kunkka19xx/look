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
        .as_deref()
        .and_then(look_qactions::descriptor)
        .into_iter()
        .collect()
}

/// A declaration may only offer a control it SUPPLIES, never a system one: a
/// block that runs `true` could otherwise show the machine's Bluetooth, and a
/// source copied off the internet could put Shut Down under a row that looks
/// like a note. Nothing can supply one yet, so today this refuses every
/// declared control and the seam waits for the one that can.
pub fn declaration_may_offer(control: &str) -> bool {
    look_qactions::descriptor(control).is_none()
}

/// The control the producing block declared, if it may offer it. Reads the
/// sources directory: a per-selection cost, never a per-keystroke one.
fn declared_control(result_id: &str) -> Option<String> {
    CandidateIdKind::source_id_of(result_id)
        .and_then(crate::sources::find_block)
        .and_then(|block| block.control)
        .filter(|control| declaration_may_offer(control))
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
}
