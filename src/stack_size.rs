//! # stack size map
//! because rust doesn't support static class member
//! so implment a map to record the stack size for
//! different function types
//!
//!
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::mem::transmute;
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
            MAP = transmute::<Box<SMap>, *const SMap>(b);
        }
    });
    unsafe { (*MAP).clone() }
}

fn align_stack(size: usize) -> usize {
    (size + 0xF) & !0xF
}

/// get the stack size for type
pub fn get_stack_size<'a, F: Any>(f: &'a F) -> usize {
    let map = get_stack_map();
    let id = (f as &Any).get_type_id();
    let rlock = map.read().unwrap();
    let size = match rlock.get(&id).map(|n| *n) {
        Some(s) => s,
        None => 0,
    };
    size
}

/// set the stack size for type
pub fn set_stack_size<'a, F: Any>(f: &'a F, size: usize) {
    let map = get_stack_map();
    let id = (f as &Any).get_type_id();
    let mut wlock = map.write().unwrap();
    wlock.insert(id, align_stack(size));
}
