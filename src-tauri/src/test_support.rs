//! Test-only coordination between tests that start processes by the dozen
//! (the ComfyUI fixtures run Python and fake servers) and tests that time
//! how fast a process ends. Under that load Windows takes seconds just to
//! start `taskkill.exe`, which says nothing about the code under test; a
//! timing test therefore runs while no process-heavy fixture is alive.

use std::sync::{Condvar, Mutex, PoisonError};

#[derive(Default)]
struct State {
    /// Process-heavy fixtures alive.
    spawning: usize,
    /// A timing test runs.
    timing: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    spawning: 0,
    timing: false,
});
static CHANGED: Condvar = Condvar::new();

/// Held by a fixture that starts processes; waits while a timing test runs.
/// Nesting in one thread is fine: a timing test only starts at zero.
pub struct Spawning(());

/// Held by a test that times process handling; waits until no
/// [`Spawning`] guard is alive and keeps new ones waiting.
pub struct Timing(());

pub fn spawning() -> Spawning {
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    while state.timing {
        state = CHANGED.wait(state).unwrap_or_else(PoisonError::into_inner);
    }
    state.spawning += 1;
    Spawning(())
}

pub fn timing() -> Timing {
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    while state.spawning > 0 || state.timing {
        state = CHANGED.wait(state).unwrap_or_else(PoisonError::into_inner);
    }
    state.timing = true;
    Timing(())
}

impl Drop for Spawning {
    fn drop(&mut self) {
        let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        state.spawning -= 1;
        CHANGED.notify_all();
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        state.timing = false;
        CHANGED.notify_all();
    }
}
