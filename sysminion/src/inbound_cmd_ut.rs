use crate::inbound_cmd::{InboundCommandClaim, InboundCommandLedger, InboundCommandState};

#[test]
fn claim_new_then_duplicate_reports_existing_state() {
    let tmp = tempfile::tempdir().unwrap();
    let ledger = InboundCommandLedger::open(tmp.path()).unwrap();

    assert_eq!(ledger.claim("mcmd|minion-1|cycle-1", "cycle-1").unwrap(), InboundCommandClaim::AcceptedNew);
    assert_eq!(ledger.claim("mcmd|minion-1|cycle-1", "cycle-1").unwrap(), InboundCommandClaim::Duplicate(InboundCommandState::Accepted));
}

#[test]
fn state_transitions_persist_across_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let ledger = InboundCommandLedger::open(tmp.path()).unwrap();
    let key = "mcmd|minion-1|cycle-1";

    assert_eq!(ledger.claim(key, "cycle-1").unwrap(), InboundCommandClaim::AcceptedNew);
    assert!(ledger.set_state(key, InboundCommandState::Running).unwrap());
    drop(ledger);

    let reopened = InboundCommandLedger::open(tmp.path()).unwrap();
    assert_eq!(reopened.state(key).unwrap(), Some(InboundCommandState::Running));
    assert!(reopened.set_state(key, InboundCommandState::Completed).unwrap());
    assert_eq!(reopened.state(key).unwrap(), Some(InboundCommandState::Completed));
}

#[test]
fn remove_forgets_command_claim() {
    let tmp = tempfile::tempdir().unwrap();
    let ledger = InboundCommandLedger::open(tmp.path()).unwrap();
    let key = "mcmd|minion-1|cycle-1";

    assert_eq!(ledger.claim(key, "cycle-1").unwrap(), InboundCommandClaim::AcceptedNew);
    assert!(ledger.remove(key).unwrap());
    assert_eq!(ledger.state(key).unwrap(), None);
}
