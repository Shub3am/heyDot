// Its own test binary: the credential store is process-global, so no other test may swap it mid-race.

use dot_settings::read_or_create_history_key;
use std::sync::Barrier;

#[test]
fn callers_racing_the_first_read_all_get_the_same_key() {
    keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
    let start_together = Barrier::new(16);

    let history_keys: Vec<String> = std::thread::scope(|scope| {
        let racers: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    start_together.wait();
                    read_or_create_history_key().unwrap()
                })
            })
            .collect();
        racers
            .into_iter()
            .map(|racer| racer.join().unwrap())
            .collect()
    });

    assert!(
        history_keys
            .iter()
            .all(|history_key| *history_key == history_keys[0])
    );
    assert_eq!(read_or_create_history_key().unwrap(), history_keys[0]);
}
