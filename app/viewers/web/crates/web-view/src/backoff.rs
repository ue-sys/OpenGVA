//! How long to wait before trying again.

use std::time::Duration;

/// The waits, in seconds: each longer than the last, and the last from then
/// on. A server that is starting is found within a second or two of being
/// there; one that is not there at all is asked four times a minute.
const WAITS: [u64; 5] = [1, 2, 4, 8, 15];

/// How long a view waits before trying again what did not work: longer each
/// time, until something does.
#[derive(Debug, Default)]
pub struct Backoff {
    /// How many times in a row it has not worked.
    failures: usize,
}

impl Backoff {
    /// It did not work: how long to wait before the next try.
    pub fn wait(&mut self) -> Duration {
        let wait = WAITS[self.failures.min(WAITS.len() - 1)];
        self.failures += 1;
        Duration::from_secs(wait)
    }

    /// It worked: the next wait is the shortest again.
    pub fn reset(&mut self) {
        self.failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_wait_is_longer_than_the_last_up_to_a_quarter_of_a_minute() {
        let mut backoff = Backoff::default();
        let waits: Vec<u64> = (0..8).map(|_| backoff.wait().as_secs()).collect();
        assert_eq!(waits, [1, 2, 4, 8, 15, 15, 15, 15]);
    }

    #[test]
    fn what_works_makes_the_next_wait_the_shortest_again() {
        let mut backoff = Backoff::default();
        for _ in 0..6 {
            backoff.wait();
        }
        backoff.reset();
        assert_eq!(backoff.wait().as_secs(), 1);
        assert_eq!(backoff.wait().as_secs(), 2);
    }
}
