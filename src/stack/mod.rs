//! # generator stack
//!
//!

use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::mem::MaybeUninit;
use std::os::raw::c_void;
use std::ptr;

#[cfg_attr(unix, path = "unix.rs")]
#[cfg_attr(windows, path = "windows.rs")]
pub mod sys;

pub use sys::overflow;

// `StackBoxHeader` is placed right in front of the data, so the data pointer
// must be aligned to at least this.
const HEADER_ALIGN: usize = std::mem::align_of::<StackBoxHeader>();
const HEADER_BYTES: usize = std::mem::size_of::<StackBoxHeader>();
// the bookkeeping counts whole words, so the header must be a whole number of
// them
const HEADER_SIZE: usize = HEADER_BYTES / std::mem::size_of::<usize>();
const _: () = assert!(HEADER_BYTES.is_multiple_of(std::mem::size_of::<usize>()));

struct StackBoxHeader {
    // track the stack
    stack: Stack,
    // track how big the data is (in usize)
    data_size: usize,
    // non zero dealloc the stack
    need_drop: usize,
}

/// Compute the addresses of a block of `size` bytes placed below `old_end`.
///
/// The data pointer is aligned down to `align` (which must be a power of two)
/// and the header is placed right in front of it. Returns `(data_addr,
/// header_addr)`, or `None` if the block cannot be placed above address 0.
#[inline]
fn block_addresses(old_end: usize, size: usize, align: usize) -> Option<(usize, usize)> {
    debug_assert!(align.is_power_of_two());
    let data_addr = old_end.checked_sub(size)? & !(align - 1);
    let header_addr = data_addr.checked_sub(HEADER_BYTES)?;
    Some((data_addr, header_addr))
}

/// A pointer type for stack allocation.
pub struct StackBox<T> {
    // the stack memory
    ptr: ptr::NonNull<T>,
}

impl<T> StackBox<T> {
    /// create uninit stack box
    fn new_uninit(stack: &mut Stack, need_drop: usize) -> MaybeUninit<Self> {
        // cheat #[warn(clippy::needless_pass_by_ref_mut)]
        // we need mutable ref for ownership
        let _ = stack as *mut Stack;

        let offset = unsafe { &mut *stack.get_offset() };
        let layout = std::alloc::Layout::new::<T>();
        let align = std::cmp::max(layout.align(), HEADER_ALIGN);
        let word = std::mem::size_of::<usize>();

        // Align the data using the real address instead of assuming `top` is
        // `align` aligned: `top` is only guaranteed to be page aligned and
        // `align` may exceed the page size. The data is placed as low as
        // possible while staying aligned; everything between its end and the
        // old end of the used area is padding.
        let top = stack.buf.top as usize;
        let old_end = top - *offset * word;
        let needed = layout.size().div_ceil(word).saturating_add(HEADER_SIZE);
        let (data_addr, header_addr) = match block_addresses(old_end, layout.size(), align) {
            Some(addresses) => addresses,
            None => Self::alloc_overflow(needed, stack.size()),
        };

        // The data plus its header must fit into the stack. Check this before
        // mutating `*offset`: without the check, a `T` bigger than the stack
        // (for example a closure capturing a large array) would place the
        // block below the bottom of the mapping and the writes below would go
        // out of bounds instead of failing cleanly.
        let data_size = (old_end - data_addr) / word;
        let end_offset = *offset + data_size + HEADER_SIZE;
        if end_offset > stack.size() {
            Self::alloc_overflow(data_size.saturating_add(HEADER_SIZE), stack.size());
        }

        // Commit the reservation (data + header) in one step.
        *offset = end_offset;
        let ptr = unsafe { ptr::NonNull::new_unchecked(data_addr as *mut T) };

        // init the header
        unsafe {
            let header = &mut *(header_addr as *mut StackBoxHeader);
            header.data_size = data_size;
            header.need_drop = need_drop;
            header.stack = stack.shadow_clone();
            MaybeUninit::new(StackBox { ptr })
        }
    }

    /// Panic with a clear message when a block does not fit into the stack.
    #[cold]
    #[inline(never)]
    fn alloc_overflow(needed: usize, capacity: usize) -> ! {
        panic!(
            "StackBox<{}> allocation exceeds stack: {} words needed, {} words capacity",
            std::any::type_name::<T>(),
            needed,
            capacity
        );
    }

    fn get_header(&self) -> &StackBoxHeader {
        unsafe {
            let header = (self.ptr.as_ptr() as *mut usize).offset(0 - HEADER_SIZE as isize);
            &*(header as *const StackBoxHeader)
        }
    }

    /// move data into the box
    pub(crate) unsafe fn init(&mut self, data: T) {
        ptr::write(self.ptr.as_ptr(), data);
    }

    // get the stack ptr
    pub(crate) fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// Constructs a StackBox from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function is unsafe because improper use may lead to
    /// memory problems. For example, a double-free may occur if the
    /// function is called twice on the same raw pointer.
    #[inline]
    pub(crate) unsafe fn from_raw(raw: *mut T) -> Self {
        StackBox {
            ptr: ptr::NonNull::new_unchecked(raw),
        }
    }

    // Consumes the `StackBox`, returning a wrapped raw pointer.
    // #[inline]
    // pub(crate) fn into_raw(b: StackBox<T>) -> *mut T {
    //     let ret = b.ptr.as_ptr();
    //     std::mem::forget(b);
    //     ret
    // }
}

pub struct Func {
    data: *mut (),
    size: usize,
    offset: *mut usize,
    func: fn(*mut ()),
    drop: fn(*mut ()),
}

impl Func {
    pub fn call_once(mut self) {
        let data = self.data;
        self.data = ptr::null_mut();
        (self.func)(data);
    }
}

impl Drop for Func {
    fn drop(&mut self) {
        if !self.data.is_null() {
            (self.drop)(self.data);
        }
        unsafe { *self.offset -= self.size };
    }
}

impl<F: FnOnce()> StackBox<F> {
    fn call_once(data: *mut ()) {
        unsafe {
            let data = data as *mut F;
            let f = data.read();
            f();
        }
    }

    fn drop_inner(data: *mut ()) {
        unsafe {
            let data = data as *mut F;
            ptr::drop_in_place(data);
        }
    }

    /// create a functor on the stack
    pub(crate) fn new_fn_once(stack: &mut Stack, data: F) -> Func {
        unsafe {
            let mut d = Self::new_uninit(stack, 0);
            (*d.as_mut_ptr()).init(data);
            let d = d.assume_init();
            let header = d.get_header();
            let f = Func {
                data: d.ptr.as_ptr() as *mut (),
                size: header.data_size + HEADER_SIZE,
                offset: stack.get_offset(),
                func: Self::call_once,
                drop: Self::drop_inner,
            };
            std::mem::forget(d);
            f
        }
    }
}

impl<T> std::ops::Deref for StackBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> std::ops::DerefMut for StackBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr.as_mut() }
    }
}

/// Restores the bump offset of a `StackBox` when it goes away, even if
/// `T::drop` panics.
struct OffsetGuard {
    offset: *mut usize,
    size: usize,
}

impl Drop for OffsetGuard {
    fn drop(&mut self) {
        unsafe { *self.offset -= self.size }
    }
}

/// Releases the stack memory once the owning `StackBox` is gone, even if
/// `T::drop` panics.
struct DeallocGuard(Stack);

#[cfg(test)]
thread_local! {
    static DEALLOC_GUARD_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Drop for DeallocGuard {
    fn drop(&mut self) {
        self.0.drop_stack();
        #[cfg(test)]
        DEALLOC_GUARD_DROPS.with(|drops| drops.set(drops.get() + 1));
    }
}

impl<T> Drop for StackBox<T> {
    fn drop(&mut self) {
        // Read the header before dropping `T`: `T::drop` may run arbitrary
        // code (a generator destructor resumes the coroutine), so no
        // reference into the stack may stay alive across it.
        let header = self.get_header();
        let data_size = header.data_size;
        let need_drop = header.need_drop != 0;
        let stack = header.stack.shadow_clone();

        // Keep the box reserved while `T` is destroyed, so any allocation a
        // destructor performs lands below this block instead of on top of it.
        // Guards make both the offset restore and the deallocation happen
        // even when `T::drop` unwinds. `_restore` is declared last, so it is
        // dropped first (normal and unwinding paths), before the mapping is
        // released.
        let _dealloc = if need_drop {
            Some(DeallocGuard(stack.shadow_clone()))
        } else {
            None
        };
        let _restore = OffsetGuard {
            offset: stack.get_offset(),
            size: data_size + HEADER_SIZE,
        };

        unsafe { ptr::drop_in_place(self.ptr.as_ptr()) };
    }
}

/// Error type returned by stack allocation methods.
#[derive(Debug)]
pub enum StackError {
    /// Contains the maximum amount of memory allowed to be allocated as stack space.
    ExceedsMaximumSize(usize),

    /// Returned if some kind of I/O error happens during allocation.
    IoError(io::Error),
}

impl Display for StackError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            StackError::ExceedsMaximumSize(size) => write!(
                fmt,
                "Requested more than max size of {size} bytes for a stack"
            ),
            StackError::IoError(ref e) => e.fmt(fmt),
        }
    }
}

impl Error for StackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            StackError::ExceedsMaximumSize(_) => None,
            StackError::IoError(ref e) => Some(e),
        }
    }
}

/// Represents any kind of stack memory.
///
/// `FixedSizeStack` as well as `ProtectedFixedSizeStack`
/// can be used to allocate actual stack space.
#[derive(Debug)]
pub struct SysStack {
    top: *mut c_void,
    bottom: *mut c_void,
}

impl SysStack {
    /// Creates a (non-owning) representation of some stack memory.
    ///
    /// It is unsafe because it is your responsibility to make sure that `top` and `bottom` are valid
    /// addresses.
    #[inline]
    pub unsafe fn new(top: *mut c_void, bottom: *mut c_void) -> SysStack {
        debug_assert!(top >= bottom);

        SysStack { top, bottom }
    }

    /// Returns the top of the stack from which on it grows downwards towards bottom().
    #[inline]
    pub fn top(&self) -> *mut c_void {
        self.top
    }

    /// Returns the bottom of the stack and thus it's end.
    #[inline]
    pub fn bottom(&self) -> *mut c_void {
        self.bottom
    }

    /// Returns the size of the stack between top() and bottom().
    #[inline]
    pub fn len(&self) -> usize {
        self.top as usize - self.bottom as usize
    }

    /// Returns the minimal stack size allowed by the current platform.
    #[inline]
    pub fn min_size() -> usize {
        sys::min_stack_size()
    }

    /// Allocates a new stack of `size`.
    fn allocate(mut size: usize, protected: bool) -> Result<SysStack, StackError> {
        let page_size = sys::page_size();
        let min_stack_size = sys::min_stack_size();
        let max_stack_size = sys::max_stack_size();
        let add_shift = i32::from(protected);
        let add = page_size << add_shift;

        if size < min_stack_size {
            size = min_stack_size;
        }

        size = (size - 1) & !(page_size.overflowing_sub(1).0);

        if let Some(size) = size.checked_add(add) {
            if size <= max_stack_size {
                let mut ret = unsafe { sys::allocate_stack(size) };

                if protected {
                    if let Ok(stack) = ret {
                        ret = unsafe { sys::protect_stack(&stack) };
                    }
                }

                return ret.map_err(StackError::IoError);
            }
        }

        Err(StackError::ExceedsMaximumSize(max_stack_size - add))
    }
}

unsafe impl Send for SysStack {}

/// Sentinel word used to mark the unused (bottom) part of a stack so that
/// `Stack::get_used_size` can estimate how much of it has been touched.
///
/// Both the writer (`Stack::new`) and the reader (`Stack::get_used_size`) use
/// this same constant, so they always agree regardless of word size or
/// endianness. A fairly high-entropy pattern is used to reduce (though not
/// eliminate) the chance that real stack data happens to look untouched.
const STACK_SENTINEL: usize = 0xDEAD_BEEFusize;

/// generator stack
/// this struct will not dealloc the memory
/// instead StackBox<> would track it's usage and dealloc it
pub struct Stack {
    buf: SysStack,
}

impl Stack {
    /// Allocate a new stack of `size` * size_of::<usize>() bytes. If size = 0,
    /// this is a `dummy_stack`
    pub fn new(size: usize) -> Stack {
        let track = (size & 1) != 0;

        let bytes = usize::max(size * std::mem::size_of::<usize>(), SysStack::min_size());

        let buf = SysStack::allocate(bytes, true).expect("failed to alloc sys stack");

        let stk = Stack { buf };

        // if size is not even we do the full foot print test
        let count = if track {
            stk.size()
        } else {
            // we only check the last few words
            8
        };

        // fill the bottom `count` words with the sentinel word. Filling whole
        // words (instead of raw bytes) keeps the written pattern identical to
        // what `get_used_size` compares against, on any word size/endianness.
        unsafe {
            let mut p = stk.buf.bottom as *mut usize;
            for _ in 0..count {
                p.write(STACK_SENTINEL);
                p = p.offset(1);
            }
        }

        // init the stack box usage
        let offset = stk.get_offset();
        unsafe { *offset = 1 };

        stk
    }

    /// get used stack size
    pub fn get_used_size(&self) -> usize {
        let cap = self.size();
        // count how many words at the bottom of the stack still hold the
        // sentinel, i.e. are untouched. The loop is bounded by `cap` so that it
        // can never read past the top of the stack (the sentinel is only
        // written to at most `cap` words by `Stack::new`).
        let mut free: usize = 0;
        unsafe {
            let mut ptr = self.buf.bottom as *const usize;
            while free < cap && *ptr == STACK_SENTINEL {
                free += 1;
                ptr = ptr.offset(1);
            }
        }
        cap - free
    }

    /// get the stack cap
    #[inline]
    pub fn size(&self) -> usize {
        self.buf.len() / std::mem::size_of::<usize>()
    }

    /// Point to the high end of the allocated stack
    pub fn end(&self) -> *mut usize {
        let offset = self.get_offset();
        unsafe { (self.buf.top as *mut usize).offset(0 - *offset as isize) }
    }

    /// Point to the low end of the allocated stack
    pub fn begin(&self) -> *mut usize {
        self.buf.bottom as *mut _
    }

    /// alloc buffer on this stack
    pub fn alloc_uninit_box<T>(&mut self) -> MaybeUninit<StackBox<T>> {
        // the first obj should set need drop to non zero
        StackBox::<T>::new_uninit(self, 1)
    }

    // get offset
    fn get_offset(&self) -> *mut usize {
        unsafe { (self.buf.top as *mut usize).offset(-1) }
    }

    // dealloc the stack
    fn drop_stack(&self) {
        if self.buf.len() == 0 {
            return;
        }
        let page_size = sys::page_size();
        let guard = (self.buf.bottom as usize - page_size) as *mut c_void;
        let size_with_guard = self.buf.len() + page_size;
        unsafe {
            sys::deallocate_stack(guard, size_with_guard);
        }
    }

    fn shadow_clone(&self) -> Self {
        Stack {
            buf: SysStack {
                top: self.buf.top,
                bottom: self.buf.bottom,
            },
        }
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let offset = self.get_offset();
        write!(f, "Stack<{:?}, Offset={}>", self.buf, unsafe { *offset })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_addresses_align_to_the_real_address() {
        const ALIGN: usize = 8192;
        // `mmap` only guarantees page alignment, so a stack top can look like
        // this even when the data requires a bigger alignment.
        let old_end = 0x1000_1000;
        assert_ne!(old_end % ALIGN, 0);

        let (data_addr, header_addr) = block_addresses(old_end, 100, ALIGN).unwrap();

        assert_eq!(data_addr % ALIGN, 0);
        assert!(data_addr + 100 <= old_end);
        assert_eq!(header_addr, data_addr - HEADER_BYTES);
        assert_eq!(header_addr % HEADER_ALIGN, 0);
    }

    #[test]
    fn block_addresses_reject_blocks_below_address_zero() {
        assert!(block_addresses(HEADER_BYTES - 1, 0, HEADER_ALIGN).is_none());
    }

    #[repr(C, align(8192))]
    struct OverAligned([u8; 8]);

    #[test]
    fn stack_box_aligns_over_aligned_data() {
        let mut stack = Stack::new(0x2000);
        let mut uninit = stack.alloc_uninit_box::<OverAligned>();
        unsafe { (*uninit.as_mut_ptr()).init(OverAligned([1; 8])) };
        let boxed = unsafe { uninit.assume_init() };

        assert_eq!(
            boxed.as_ptr() as usize % std::mem::align_of::<OverAligned>(),
            0
        );
    }

    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("panic while dropping a stack box");
        }
    }

    #[test]
    fn stack_box_restores_offset_when_drop_panics() {
        let mut stack = Stack::new(0x100);
        let offset = unsafe { *stack.get_offset() };

        let mut uninit = StackBox::<PanicOnDrop>::new_uninit(&mut stack, 0);
        unsafe { (*uninit.as_mut_ptr()).init(PanicOnDrop) };
        let boxed = unsafe { uninit.assume_init() };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(boxed)));
        assert!(result.is_err());
        assert_eq!(unsafe { *stack.get_offset() }, offset);

        // hand the stack to an owning box so the mapping is released
        let mut owner = stack.alloc_uninit_box::<u64>();
        unsafe { (*owner.as_mut_ptr()).init(0) };
        drop(unsafe { owner.assume_init() });
    }

    #[test]
    fn stack_box_frees_stack_when_drop_panics() {
        let mut stack = Stack::new(0x100);

        let mut uninit = stack.alloc_uninit_box::<PanicOnDrop>();
        unsafe { (*uninit.as_mut_ptr()).init(PanicOnDrop) };
        let boxed = unsafe { uninit.assume_init() };

        let deallocations_before = DEALLOC_GUARD_DROPS.with(std::cell::Cell::get);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(boxed)));
        assert!(result.is_err());

        let deallocations_after = DEALLOC_GUARD_DROPS.with(std::cell::Cell::get);
        assert_eq!(deallocations_after, deallocations_before + 1);
    }

    struct AllocOnDrop {
        stack: Stack,
        // points at a `Cell` in the test's frame, outside the stack memory, so
        // a broken drop order that overwrites this object cannot hide itself
        out: *const std::cell::Cell<Option<usize>>,
    }

    impl Drop for AllocOnDrop {
        fn drop(&mut self) {
            // Copy everything needed to locals before allocating: with a
            // broken drop order the nested allocation overwrites this object.
            let mut stack = self.stack.shadow_clone();
            let out = self.out;

            // Same-stack allocation from a destructor: it must land below the
            // box being dropped, which is still reserved at this point.
            let mut uninit = StackBox::<u64>::new_uninit(&mut stack, 0);
            unsafe { (*uninit.as_mut_ptr()).init(0xAA) };
            let nested = unsafe { uninit.assume_init() };
            unsafe { (*out).set(Some(nested.as_ptr() as usize)) };
        }
    }

    #[test]
    fn stack_box_stays_reserved_while_dropping() {
        let mut stack = Stack::new(0x100);
        let mut uninit = stack.alloc_uninit_box::<AllocOnDrop>();
        let self_addr = unsafe { (*uninit.as_mut_ptr()).as_ptr() as usize };
        let self_end = self_addr + std::mem::size_of::<AllocOnDrop>();
        let nested_addr = std::cell::Cell::new(None);

        unsafe {
            (*uninit.as_mut_ptr()).init(AllocOnDrop {
                stack,
                out: &nested_addr,
            })
        };
        drop(unsafe { uninit.assume_init() });

        let nested_start = nested_addr.get().expect("destructor did not run");
        let nested_end = nested_start + std::mem::size_of::<u64>();
        assert!(
            nested_end <= self_addr || nested_start >= self_end,
            "nested allocation {nested_start:#x}..{nested_end:#x} overlaps \
             the box being dropped {self_addr:#x}..{self_end:#x}"
        );
    }
}
