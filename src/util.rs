use std::sync::Mutex;

#[inline(always)]
pub fn locked<T, R>(lock: &Mutex<T>, f: impl FnOnce(&mut T) -> R) -> R {
    match lock.lock() {
        Ok(ref mut guard) => f(guard),
        Err(ref mut poisoned) => f(poisoned.get_mut()),
    }
}
