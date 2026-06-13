use libsysinspect::journal::Journal;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn temp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "libsysinspect-journal-int-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
        label
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_with_retry(path: &std::path::Path, max_bytes: u64) -> Journal {
    let mut last_err = None;
    for _ in 0..20 {
        match Journal::open(path, max_bytes) {
            Ok(journal) => return journal,
            Err(err) => {
                last_err = Some(err);
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    panic!("failed to reopen journal after bounded retries: {:?}", last_err);
}

#[test]
fn concurrent_appends_across_threads() {
    let dir = temp_dir("concurrent");
    let j = Arc::new(Journal::open(&dir, 0).unwrap());
    let cycles: usize = 50;
    let threads: usize = 4;

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let j = Arc::clone(&j);
            thread::spawn(move || {
                for c in 0..cycles {
                    let cid = format!("t{}-c{}", t, c);
                    for e in 0..10 {
                        j.append(&cid, format!("entry-{}", e).as_bytes()).unwrap();
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let pending = j.pending().unwrap();
    assert_eq!(pending.len(), threads * cycles);
    for (_, entries) in &pending {
        assert_eq!(entries.len(), 10);
    }

    drop(pending);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn crash_recovery_after_append() {
    let dir = temp_dir("crash");
    {
        let j = Journal::open(&dir, 0).unwrap();
        for i in 0..50u16 {
            j.append(&format!("c-{:04}", i), b"data").unwrap();
        }
        // Simulate crash: drop without ack
    }
    {
        let j = open_with_retry(&dir, 0);
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 50);
        assert_eq!(pending[0].0, "c-0000");
        assert_eq!(pending[49].0, "c-0049");

        // Ack first 25
        for i in 0..25u16 {
            j.ack_cycle(&format!("c-{:04}", i)).unwrap();
        }
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 25);
        assert_eq!(pending[0].0, "c-0025");
    }
    {
        // Reopen: only 25 remain
        let j = open_with_retry(&dir, 0);
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 25);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn heavy_budget_pressure() {
    let dir = temp_dir("pressure");
    let j = Journal::open(&dir, 500).unwrap();

    // 200 cycles × 10 bytes = 2000 bytes. Budget 500.
    // Should keep ~50 newest cycles.
    for i in 0..200u16 {
        j.append(&format!("c-{:04}", i), b"0123456789").unwrap();
    }

    let pending = j.pending().unwrap();
    assert!(pending.len() >= 40, "expected at least 40 survivors under budget pressure, got {}", pending.len());
    assert!(pending.len() <= 60, "expected at most 60 survivors, got {}", pending.len());

    // Verify newest cycles survived (last 50 should still be here)
    let survivor_ids: Vec<&str> = pending.iter().map(|(c, _)| c.as_str()).collect();
    for i in (170..200u16).rev() {
        let expected = format!("c-{:04}", i);
        assert!(survivor_ids.contains(&expected.as_str()), "newest cycle {} should survive budget pressure", expected);
    }

    drop(pending);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rapid_ack_storm() {
    let dir = temp_dir("ackstorm");
    let j = Journal::open(&dir, 0).unwrap();

    // Append 500 cycles with 3 entries each
    for i in 0..500u16 {
        let cid = format!("c-{:04}", i);
        j.append(&cid, b"a").unwrap();
        j.append(&cid, b"b").unwrap();
        j.append(&cid, b"c").unwrap();
    }

    assert_eq!(j.pending().unwrap().len(), 500);

    // Ack all of them rapidly
    for i in 0..500u16 {
        j.ack_cycle(&format!("c-{:04}", i)).unwrap();
    }

    assert!(j.pending().unwrap().is_empty());
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mixed_read_write_under_load() {
    let dir = temp_dir("mixed");
    let j = Arc::new(Journal::open(&dir, 0).unwrap());

    let writer = {
        let j = Arc::clone(&j);
        thread::spawn(move || {
            for i in 0..200u16 {
                j.append(&format!("w-{:04}", i), b"write").unwrap();
            }
        })
    };

    let acker = {
        let j = Arc::clone(&j);
        thread::spawn(move || {
            for _ in 0..10 {
                let pending = j.pending().unwrap();
                if let Some((cid, _)) = pending.first() {
                    let cid = cid.clone();
                    drop(pending);
                    j.ack_cycle(&cid).unwrap();
                }
            }
        })
    };

    let reader = {
        let j = Arc::clone(&j);
        thread::spawn(move || {
            for _ in 0..20 {
                let _ = j.pending().unwrap();
                thread::sleep(std::time::Duration::from_micros(100));
            }
        })
    };

    writer.join().unwrap();
    acker.join().unwrap();
    reader.join().unwrap();

    let pending = j.pending().unwrap();
    // Some acked, some remain. All should be writer's cycles.
    for (cid, _) in &pending {
        assert!(cid.starts_with("w-"));
    }

    drop(pending);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopen_preserves_all_state_across_multiple_sessions() {
    let dir = temp_dir("reopen");
    let inputs: Vec<(&str, &[u8])> = vec![
        ("session1", b"first"),
        ("session1", b"second"),
        ("session2", b"alpha"),
        ("session2", b"beta"),
        ("session2", b"gamma"),
        ("session3", b"only"),
    ];

    // Session 1: append session1 data + session2 data, ack session1
    {
        let j = Journal::open(&dir, 0).unwrap();
        j.append("session1", b"first").unwrap();
        j.append("session1", b"second").unwrap();
        j.append("session2", b"alpha").unwrap();
        j.ack_cycle("session1").unwrap();
    }

    // Session 2: append more to session2, add session3
    {
        let j = Journal::open(&dir, 0).unwrap();
        j.append("session2", b"beta").unwrap();
        j.append("session2", b"gamma").unwrap();
        j.append("session3", b"only").unwrap();
        // Don't ack anything
    }

    // Session 3: verify all remaining
    {
        let j = Journal::open(&dir, 0).unwrap();
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 2, "expected session2 and session3");
        assert_eq!(pending[0].0, "session2");
        assert_eq!(pending[0].1.len(), 3);
        assert_eq!(pending[1].0, "session3");
        assert_eq!(pending[1].1.len(), 1);

        // Ack session2, keep session3
        j.ack_cycle("session2").unwrap();
        let pending2 = j.pending().unwrap();
        assert_eq!(pending2.len(), 1);
        assert_eq!(pending2[0].0, "session3");
    }

    // Session 4: only session3 remains
    {
        let j = Journal::open(&dir, 0).unwrap();
        let pending = j.pending().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "session3");
    }

    drop(inputs);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn large_payloads_stress() {
    let dir = temp_dir("large");
    let j = Journal::open(&dir, 64 * 1024).unwrap(); // 64 KiB budget

    // Append many medium payloads
    for i in 0..100u16 {
        let payload = vec![(i % 256) as u8; 512]; // 512 bytes each
        j.append(&format!("c-{}", i), &payload).unwrap();
    }
    // 100 × 512 = 51200 bytes under 64 KiB budget — none evicted
    assert_eq!(j.pending().unwrap().len(), 100);

    // Now blow past the budget
    let huge = vec![0u8; 32 * 1024]; // 32 KiB
    j.append("huge", &huge).unwrap();
    // 51200 + 32768 = 83968 > 65536. ~36 oldest cycles evicted to make room.
    let pending = j.pending().unwrap();
    assert!(pending.len() < 70, "expected fewer than 70 cycles, got {}", pending.len());
    assert!(pending.iter().any(|(c, _)| c == "huge"), "huge cycle should survive");

    drop(pending);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}
