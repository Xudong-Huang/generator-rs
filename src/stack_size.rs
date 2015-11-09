//! # stack size map
//! because rust doesn't support static class member
//! so implment a map to record the stack size for
//! different function types
//!
//!
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::{Once, ONCE_INIT};

type SMap = Arc<RwLock<HashMap<TypeId, usize>>>;

#[inline(always)]
#[allow(dead_code)]
fn get_stack_map() -> SMap {
    static mut MAP: *const SMap = 0 as *const SMap;
    static ONCE: Once = ONCE_INIT;
    ONCE.call_once(|| {
        unsafe {
            let b: Box<SMap> = Box::new(Arc::new(RwLock::new(HashMap::new())));
            MAP = Box::into_raw(b);
        }
    });
    unsafe { (*MAP).clone() }
}

fn align_stack(size: usize) -> usize {
    (size + 0xF) & !0xF
}

/// get the stack size for type
pub fn get_stack_size<F: Any>() -> usize {
    let map = get_stack_map();
    let id = TypeId::of::<F>();
    let rlock = map.read().unwrap();
    let size = match rlock.get(&id).map(|n| *n) {
        Some(s) => s,
        None => 0,
    };
    size
}

/// set the stack size for type
pub fn set_stack_size<F: Any>(size: usize) {
    let map = get_stack_map();
    let id = TypeId::of::<F>();
    let mut wlock = map.write().unwrap();

    let saved_size = match wlock.get(&id).map(|n| *n) {
        Some(s) => s,
        None => 0,
    };

    let size = align_stack(size);

    if size > saved_size {
        wlock.insert(id, size);
    }
}
