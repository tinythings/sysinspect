use crate::journal::Journal;

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "libsysinspect-journal-ut-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn append_returns_per_cycle_sequence() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    assert_eq!(j.append("c1", b"a").unwrap(), 0);
    assert_eq!(j.append("c1", b"b").unwrap(), 1);
    assert_eq!(j.append("c2", b"x").unwrap(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ack_cycle_deletes_entries() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.append("c1", b"hello").unwrap();
    j.append("c1", b"world").unwrap();
    j.append("c2", b"keep").unwrap();
    j.ack_cycle("c1").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, "c2");
    assert_eq!(pending[0].1[0].1, b"keep");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ack_is_idempotent() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.append("c1", b"x").unwrap();
    j.ack_cycle("c1").unwrap();
    j.ack_cycle("c1").unwrap();
    assert!(j.pending().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pending_empty_on_fresh_journal() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    assert!(j.pending().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn evicts_oldest_cycle_when_over_budget() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 20).unwrap();
    j.append("c1", b"1234567890").unwrap();
    j.append("c1", b"abcdefghij").unwrap();
    j.append("c2", b"overflow!").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, "c2");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn state_persists_across_reopen() {
    let dir = temp_dir();
    let j1 = Journal::open(&dir, 0).unwrap();
    j1.append("c1", b"lost").unwrap();
    drop(j1);
    let j2 = Journal::open(&dir, 0).unwrap();
    let pending = j2.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, "c1");
    assert_eq!(pending[0].1[0].1, b"lost");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn multi_cycle_ack_partial() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.append("c1", b"a").unwrap();
    j.append("c1", b"b").unwrap();
    j.append("c2", b"x").unwrap();
    j.append("c3", b"1").unwrap();
    j.append("c3", b"2").unwrap();
    j.ack_cycle("c2").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].0, "c1");
    assert_eq!(pending[1].0, "c3");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ack_before_append_is_noop() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.ack_cycle("never_written").unwrap();
    assert!(j.pending().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pending_preserves_insertion_order() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.append("c1", b"first").unwrap();
    j.append("c2", b"second").unwrap();
    j.append("c1", b"third").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].0, "c1");
    assert_eq!(pending[0].1.len(), 2);
    assert_eq!(pending[1].0, "c2");
    assert_eq!(pending[1].1.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopen_after_partial_ack_preserves_survivors() {
    let dir = temp_dir();
    {
        let j = Journal::open(&dir, 0).unwrap();
        j.append("c1", b"keep").unwrap();
        j.append("c2", b"drop").unwrap();
        j.ack_cycle("c2").unwrap();
    }
    {
        let j = Journal::open(&dir, 0).unwrap();
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "c1");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn budget_applies_after_reopen() {
    let dir = temp_dir();
    {
        let j = Journal::open(&dir, 20).unwrap();
        j.append("c1", b"1234567890").unwrap();
    }
    {
        let j = Journal::open(&dir, 20).unwrap();
        j.append("c1", b"abcdefghij").unwrap();
        j.append("c2", b"overflow!").unwrap();
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "c2");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- edge cases ----

#[test]
fn empty_cycle_id_is_valid() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    let s = j.append("", b"data").unwrap();
    assert_eq!(s, 0);
    let pending = j.pending().unwrap();
    assert_eq!(pending[0].0, "");
    assert_eq!(pending[0].1[0].1, b"data");
    j.ack_cycle("").unwrap();
    assert!(j.pending().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_payload_is_valid() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.append("c", b"").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending[0].1[0].1.len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn payload_exactly_at_budget_does_not_evict() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 10).unwrap();
    j.append("c1", b"1234567890").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn eviction_wipes_only_cycle_when_budget_exceeded() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 5).unwrap();
    j.append("only", b"1234567890").unwrap();
    assert!(j.pending().unwrap().is_empty(), "single cycle over budget should be evicted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_after_ack_reuses_cycle_id() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.append("c1", b"first").unwrap();
    j.ack_cycle("c1").unwrap();
    let s = j.append("c1", b"second").unwrap();
    assert_eq!(s, 1);
    let pending = j.pending().unwrap();
    assert_eq!(pending[0].1[0].1, b"second");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cycle_id_with_colons() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    j.append("a:b:c", b"x").unwrap();
    j.append("a:b:c", b"y").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending[0].0, "a:b:c");
    assert_eq!(pending[0].1.len(), 2);
    j.ack_cycle("a:b:c").unwrap();
    assert!(j.pending().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn very_long_cycle_id() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    let long = "x".repeat(1024);
    j.append(&long, b"data").unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending[0].1[0].1, b"data");
    j.ack_cycle(&long).unwrap();
    assert!(j.pending().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn multi_entry_cycle_ordering_within_cycle() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    for i in 0..50u8 {
        j.append("c", &[i]).unwrap();
    }
    let pending = j.pending().unwrap();
    assert_eq!(pending[0].1.len(), 50);
    for (i, (seq, payload)) in pending[0].1.iter().enumerate() {
        assert_eq!(*seq, i as u64);
        assert_eq!(payload, &[i as u8]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- stress / integration tests ----

#[test]
fn many_cycles_with_many_entries() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    for ci in 0..200u16 {
        let cid = format!("cycle-{:04}", ci);
        for ei in 0..5 {
            j.append(&cid, format!("entry-{}", ei).as_bytes()).unwrap();
        }
    }
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 200);
    assert_eq!(pending[0].0, "cycle-0000");
    assert_eq!(pending[199].0, "cycle-0199");
    for (_, entries) in &pending {
        assert_eq!(entries.len(), 5);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn interleaved_append_and_ack() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 0).unwrap();
    for i in 0..100u16 {
        let cid = format!("c-{}", i);
        j.append(&cid, b"data").unwrap();
        j.append(&cid, b"more").unwrap();
        if i % 2 == 0 && i > 0 {
            let prev = format!("c-{}", i - 1);
            j.ack_cycle(&prev).unwrap();
        }
    }
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 51);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn budget_pressure_many_tiny_cycles() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 200).unwrap();
    for i in 0..50u16 {
        let cid = format!("cycle-{:04}", i);
        j.append(&cid, b"0123456789").unwrap();
    }
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 20);
    assert_eq!(pending[0].0, "cycle-0030");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn large_payload_near_budget_single_entry_causes_eviction() {
    let dir = temp_dir();
    let j = Journal::open(&dir, 50).unwrap();
    j.append("alpha", b"1234567890").unwrap();
    j.append("beta",  b"1234567890").unwrap();
    j.append("gamma", b"1234567890").unwrap();
    j.append("extra", &vec![0u8; 45]).unwrap();
    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, "gamma");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopen_and_continue_appending() {
    let dir = temp_dir();
    {
        let j = Journal::open(&dir, 0).unwrap();
        j.append("c1", b"a").unwrap();
        j.append("c2", b"b").unwrap();
        j.ack_cycle("c1").unwrap();
    }
    {
        let j = Journal::open(&dir, 0).unwrap();
        j.append("c2", b"c").unwrap();
        j.append("c3", b"d").unwrap();
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].0, "c2");
        assert_eq!(pending[0].1.len(), 2);
        assert_eq!(pending[1].0, "c3");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
