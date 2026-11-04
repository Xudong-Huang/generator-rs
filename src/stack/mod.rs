//! # generator stack
//!
//!

use std::cell::Cell;
use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::os::raw::c_void;
use std::ptr;
use std::rc::Rc;

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

// Words `initialize_call_frame` writes (or leaves as linkage area) just below
// `Stack::end()`, per architecture.
cfg_select! {
    // register save area
    target_arch = "s390x" => {
        const INITIAL_FRAME_WORDS: usize = 160 / std::mem::size_of::<usize>();
    }
    // back chain, CR, LR and TOC save words
    target_arch = "powerpc64" => {
        const INITIAL_FRAME_WORDS: usize = 4;
    }
    // return address and a zero word
    any(target_arch = "x86", target_arch = "x86_64") => {
        const INITIAL_FRAME_WORDS: usize = 2;
    }
    // only the stack pointer is set up
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    ) => {
        const INITIAL_FRAME_WORDS: usize = 0;
    }
}

/// Words that must stay free below the last block allocated before the
/// generator starts: the initial frame plus up to 16 bytes lost to aligning
/// the stack pointer.
const INITIAL_FRAME_RESERVE: usize = INITIAL_FRAME_WORDS + 16 / std::mem::size_of::<usize>();

struct StackBoxHeader {
    // keeps the mapped memory alive while the block exists
    stack: Stack,
    // the offset before this block was allocated
    prev_offset: usize,
    // the offset with this block allocated; it must still be the live offset
    // when the block is freed (the allocator is LIFO)
    end_offset: usize,
}

/// The header sits immediately in front of the data.
#[inline]
fn block_header(data: *const u8) -> *const StackBoxHeader {
    data.cast::<usize>().wrapping_sub(HEADER_SIZE).cast()
}

/// Compute the data address for a block of `size` bytes placed below
/// `old_end`, aligned down to `align` (which must be a power of two).
///
/// Returns `None` if the block cannot be placed above address 0.
#[inline]
fn data_address(old_end: usize, size: usize, align: usize) -> Option<usize> {
    debug_assert!(align.is_power_of_two());
    Some(old_end.checked_sub(size)? & !(align - 1))
}

/// A pointer type for stack allocation.
pub struct StackBox<T> {
    // the stack memory
    ptr: ptr::NonNull<T>,
}

impl<T> StackBox<T> {
    /// Reserve a block for `T` at the current stack end, plus `extra_words`
    /// words of bookkeeping below the data, while keeping `headroom` words
    /// free below the reservation.
    ///
    /// Returns `(data_ptr, prev_offset, end_offset)`.
    fn reserve(stack: &Stack, extra_words: usize, headroom: usize) -> (*mut u8, usize, usize) {
        let layout = std::alloc::Layout::new::<T>();
        let align = std::cmp::max(layout.align(), HEADER_ALIGN);
        let word = std::mem::size_of::<usize>();

        // Align the data using the real address instead of assuming `top` is
        // `align` aligned: `top` is only guaranteed to be page aligned and
        // `align` may exceed the page size. The data is placed as low as
        // possible while staying aligned; everything between its end and the
        // old end of the used area is padding.
        let offset = stack.offset();
        let top = stack.mem.buf.top.cast::<u8>();
        let old_end = top.wrapping_sub(offset * word);
        let old_end_addr = old_end.addr();
        let needed = layout.size().div_ceil(word).saturating_add(extra_words);
        let data_addr = match data_address(old_end_addr, layout.size(), align) {
            Some(data_addr) => data_addr,
            None => Self::alloc_overflow(needed, stack.size()),
        };
        // Derive the data pointer from `old_end` itself instead of rebuilding
        // it from an integer, so it keeps the mapping's provenance.
        let data_ptr = old_end.wrapping_sub(old_end_addr - data_addr);

        // The data plus its bookkeeping must fit into the stack, and `headroom`
        // words must remain free below it. Check this before mutating the
        // offset: without the check, a `T` bigger than the stack (for example a
        // closure capturing a large array) would place the block below the
        // bottom of the mapping and the writes below would go out of bounds
        // instead of failing cleanly.
        let prev_offset = offset;
        let data_size = (old_end_addr - data_addr) / word;
        let end_offset = prev_offset + data_size + extra_words;
        let needed = data_size
            .saturating_add(extra_words)
            .saturating_add(headroom);
        if end_offset
            .checked_add(headroom)
            .is_none_or(|end| end > stack.size())
        {
            Self::alloc_overflow(needed, stack.size());
        }

        // Commit the reservation in one step.
        stack.mem.offset.set(end_offset);
        (data_ptr, prev_offset, end_offset)
    }

    /// create uninit stack box
    fn new_uninit(stack: &Stack) -> StackBoxUninit<T> {
        let (data_ptr, prev_offset, end_offset) = Self::reserve(stack, HEADER_SIZE, 0);
        // A successful reservation guarantees that the header fits too.
        let header_ptr = data_ptr.wrapping_sub(HEADER_BYTES).cast::<StackBoxHeader>();

        // init the header. The memory is uninitialized, so it must be written
        // with `ptr::write`: assigning to `header.stack` would try to drop the
        // previous (garbage) value now that `Stack` has a destructor.
        let header = StackBoxHeader {
            stack: stack.clone(),
            prev_offset,
            end_offset,
        };
        unsafe { ptr::write(header_ptr, header) };

        let ptr = unsafe { ptr::NonNull::new_unchecked(data_ptr.cast::<T>()) };
        StackBoxUninit { ptr }
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

    fn header_ptr(&self) -> *const StackBoxHeader {
        block_header(self.ptr.as_ptr().cast())
    }

    // get the stack ptr
    pub(crate) fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// Constructs a `StackBox` from a raw pointer previously returned by
    /// `Generator::into_raw`.
    ///
    /// # Safety
    ///
    /// `raw` must point to a value allocated by [`Stack::alloc_uninit_box`] on
    /// a stack that is still alive, and its block header must be intact. The
    /// returned box takes over ownership: reconstructing two boxes from the
    /// same pointer causes a double free when both are dropped.
    #[inline]
    pub(crate) unsafe fn from_raw(raw: *mut T) -> Self {
        StackBox {
            ptr: ptr::NonNull::new_unchecked(raw),
        }
    }
}

/// A reserved stack block whose value has not been initialized yet.
///
/// Created by [`Stack::alloc_uninit_box`]. Dropping it without calling
/// [`init`](Self::init) releases the reservation without dropping a value.
pub struct StackBoxUninit<T> {
    ptr: ptr::NonNull<T>,
}

impl<T> StackBoxUninit<T> {
    /// The address of the value to initialize.
    #[cfg(test)]
    pub(crate) fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// Write `value` into the block and turn it into a [`StackBox`].
    pub(crate) fn init(self, value: T) -> StackBox<T> {
        let this = std::mem::ManuallyDrop::new(self);
        unsafe { ptr::write(this.ptr.as_ptr(), value) }
        StackBox { ptr: this.ptr }
    }
}

impl<T> Drop for StackBoxUninit<T> {
    fn drop(&mut self) {
        // The block was never initialized: release its reservation and the
        // header's stack handle, without running a destructor.
        let header = block_header(self.ptr.as_ptr().cast());
        unsafe {
            let stack = ptr::read(ptr::addr_of!((*header).stack));
            let _restore = OffsetGuard {
                offset: stack.offset_cell(),
                prev_offset: (*header).prev_offset,
                end_offset: (*header).end_offset,
                abandoned: Some(&stack.mem.abandoned),
            };
        }
    }
}

/// A closure allocated on a generator stack.
///
/// The block, and the stack control block its offset points into, are kept
/// alive by the owning generator: every `Func` lives either in
/// `GeneratorImpl.f` or in a frame of the generator's own stack, so the offset
/// is valid whenever `Func` can be dropped.
pub struct Func {
    data: *mut (),
    // points into the generator's stack control block
    offset: ptr::NonNull<Cell<usize>>,
    prev_offset: usize,
    end_offset: usize,
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
        // Restore the offset even if the closure's destructor panics, and make
        // sure the closure's block is the most recent allocation.
        let data = self.data;
        self.data = ptr::null_mut();
        let _restore = OffsetGuard {
            offset: unsafe { self.offset.as_ref() },
            prev_offset: self.prev_offset,
            end_offset: self.end_offset,
            abandoned: None,
        };
        if !data.is_null() {
            (self.drop)(data);
        }
    }
}

impl<F: FnOnce()> StackBox<F> {
    fn call_once(data: *mut ()) {
        unsafe {
            let data = data.cast::<F>();
            let f = data.read();
            f();
        }
    }

    fn drop_inner(data: *mut ()) {
        unsafe {
            let data = data.cast::<F>();
            ptr::drop_in_place(data);
        }
    }

    /// create a functor on the stack
    pub(crate) fn new_fn_once(stack: &Stack, data: F) -> Func {
        unsafe {
            // A functor block is a sub-allocation managed by `Func`: it needs
            // no header and no stack handle of its own. The generator holds
            // the handle that keeps the mapping alive whenever `Func` can be
            // dropped.
            let (data_ptr, prev_offset, end_offset) =
                Self::reserve(stack, 0, INITIAL_FRAME_RESERVE);
            ptr::write(data_ptr.cast::<F>(), data);

            Func {
                data: data_ptr.cast::<()>(),
                offset: ptr::NonNull::from(stack.offset_cell()),
                prev_offset,
                end_offset,
                func: Self::call_once,
                drop: Self::drop_inner,
            }
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
///
/// The allocator is LIFO: freeing a block restores the offset that was live
/// before it was allocated. If the offset is not the block's own end offset,
/// another allocation happened after it (or the accounting is corrupt), which
/// this guard flags in debug builds.
struct OffsetGuard<'a> {
    offset: &'a Cell<usize>,
    prev_offset: usize,
    end_offset: usize,
    // the stack's abandoned flag, for guards that belong to a stack block.
    // `Func` guards have no access to the stack and pass `None`: a functor
    // left behind by an overflow is never dropped.
    abandoned: Option<&'a Cell<bool>>,
}

impl Drop for OffsetGuard<'_> {
    fn drop(&mut self) {
        // Destructors may be skipped while unwinding (for example an
        // abandoned generator's functor), so only enforce strict LIFO
        // accounting on the normal path. A stack abandoned by an overflow
        // never unwound its frames, so later allocations are still reserved
        // and the check does not apply either.
        if !std::thread::panicking() && !self.abandoned.is_some_and(Cell::get) {
            debug_assert_eq!(
                self.offset.get(),
                self.end_offset,
                "stack block freed out of LIFO order"
            );
        }
        self.offset.set(self.prev_offset);
    }
}

impl<T> Drop for StackBox<T> {
    fn drop(&mut self) {
        // Read the header before dropping `T`: `T::drop` may run arbitrary
        // code (a generator destructor resumes the coroutine), so no
        // reference into the stack may stay alive across it.
        let header = self.header_ptr();
        let prev_offset = unsafe { (*header).prev_offset };
        let end_offset = unsafe { (*header).end_offset };
        // Take the stack handle out of the raw header: it keeps the mapping
        // alive until this drop is finished.
        let stack = unsafe { ptr::read(ptr::addr_of!((*header).stack)) };

        // Keep the box reserved while `T` is destroyed, so any allocation a
        // destructor performs lands below this block instead of on top of it.
        // The guard restores the offset even when `T::drop` unwinds; it is
        // dropped before `stack`, so the bookkeeping still happens while the
        // memory is mapped.
        let _restore = OffsetGuard {
            offset: stack.offset_cell(),
            prev_offset,
            end_offset,
            abandoned: Some(&stack.mem.abandoned),
        };

        unsafe { ptr::drop_in_place(self.ptr.as_ptr()) };
    }
}

/// Error type returned by stack allocation methods.
#[derive(Debug)]
pub enum StackError {
    /// Contains the maximum amount of memory allowed to be allocated as stack
    /// space, in bytes.
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

/// The bounds of a mapped stack region, from `bottom()` up to `top()`.
///
/// It does not own the memory; `StackMem` unmaps it when the last `Stack`
/// handle is dropped.
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

    /// Returns the bottom of the stack and thus its end.
    #[inline]
    pub fn bottom(&self) -> *mut c_void {
        self.bottom
    }

    /// Returns the size of the stack between top() and bottom().
    #[inline]
    pub fn len(&self) -> usize {
        self.top.addr() - self.bottom.addr()
    }

    /// Returns the minimal stack size allowed by the current platform.
    #[inline]
    pub fn min_size() -> usize {
        sys::min_stack_size()
    }

    /// Allocates a new stack of `size`, with a guard page at the bottom.
    fn allocate(mut size: usize) -> Result<SysStack, StackError> {
        let page_size = sys::page_size();
        let min_stack_size = sys::min_stack_size();
        let max_stack_size = sys::max_stack_size();
        // one page for the guard and one page of rounding slack
        let add = page_size << 1;

        if size < min_stack_size {
            size = min_stack_size;
        }

        // Round down to a page boundary; together with the extra pages added
        // below, the usable length becomes `size` rounded up to a whole page.
        size = (size - 1) & !(page_size - 1);

        if let Some(size) = size.checked_add(add) {
            if size <= max_stack_size {
                let stack = unsafe { sys::allocate_stack(size) }.map_err(StackError::IoError)?;

                match unsafe { sys::protect_stack(&stack) } {
                    Ok(protected) => return Ok(protected),
                    Err(err) => {
                        // The guard page could not be installed: release the
                        // mapping instead of leaking it.
                        unsafe { sys::deallocate_stack(stack.bottom(), stack.len()) };
                        return Err(StackError::IoError(err));
                    }
                }
            }
        }

        Err(StackError::ExceedsMaximumSize(
            max_stack_size.saturating_sub(add),
        ))
    }
}

/// Sentinel word used to mark the unused (bottom) part of a stack so that
/// `Stack::get_used_size` can estimate how much of it has been touched.
///
/// Both the writer (`Stack::new`) and the reader (`Stack::get_used_size`) use
/// this same constant, so they always agree regardless of word size or
/// endianness. A fairly high-entropy pattern is used to reduce (though not
/// eliminate) the chance that real stack data happens to look untouched.
const STACK_SENTINEL: usize = 0xDEAD_BEEFusize;

/// Handles to a mapped stack. The mapping is released when the last handle
/// (generator, box or header) is dropped.
///
/// The handle is deliberately thread-affine: every handle lives in the
/// generator's own allocation and moves with it, so `Generator`'s `Send` impl
/// is the only place where a stack may cross a thread boundary.
#[derive(Clone)]
pub struct Stack {
    mem: Rc<StackMem>,
}

/// The shared allocation backing a `Stack`.
struct StackMem {
    buf: SysStack,
    // bump offset in words from the top of the stack
    offset: Cell<usize>,
    // an overflow abandoned the running frames without unwinding them, so
    // allocations made by the body are still reserved
    abandoned: Cell<bool>,
}

#[cfg(test)]
thread_local! {
    static STACK_DEALLOCS: Cell<usize> = const { Cell::new(0) };
}

impl Drop for StackMem {
    fn drop(&mut self) {
        let page_size = sys::page_size();
        let guard = self
            .buf
            .bottom
            .cast::<u8>()
            .wrapping_sub(page_size)
            .cast::<c_void>();
        let size_with_guard = self.buf.len() + page_size;
        unsafe {
            sys::deallocate_stack(guard, size_with_guard);
        }
        #[cfg(test)]
        STACK_DEALLOCS.with(|drops| drops.set(drops.get() + 1));
    }
}

/// Number of words sampled at the bottom of a stack when full usage tracking
/// was not requested.
const STACK_USAGE_SAMPLE: usize = 8;

impl Stack {
    /// Allocate a new stack of `size` words (`size * size_of::<usize>()`
    /// bytes). Sizes below the platform minimum are rounded up to it.
    ///
    /// An odd `size` requests full usage tracking for
    /// [`get_used_size`](Self::get_used_size); even sizes only sample the
    /// bottom [`STACK_USAGE_SAMPLE`] words to keep creation cheap.
    pub fn new(size: usize) -> Stack {
        let track_usage = (size & 1) != 0;

        let bytes = size
            .checked_mul(std::mem::size_of::<usize>())
            .unwrap_or_else(|| panic!("requested stack size {size} words overflows usize"))
            .max(SysStack::min_size());

        let buf = SysStack::allocate(bytes).expect("failed to alloc sys stack");

        let stk = Stack {
            mem: Rc::new(StackMem {
                buf,
                offset: Cell::new(0),
                abandoned: Cell::new(false),
            }),
        };

        // Full tracking touches every stack page, so only do it when
        // requested; otherwise sample the bottom words, which is enough to
        // notice a stack that grew all the way down.
        let count = if track_usage {
            stk.size()
        } else {
            STACK_USAGE_SAMPLE
        };

        // fill the bottom `count` words with the sentinel word. Filling whole
        // words (instead of raw bytes) keeps the written pattern identical to
        // what `get_used_size` compares against, on any word size/endianness.
        unsafe {
            let mut p = stk.mem.buf.bottom.cast::<usize>();
            for _ in 0..count {
                p.write(STACK_SENTINEL);
                p = p.offset(1);
            }
        }

        stk
    }

    /// Estimate how many words of the stack were used.
    ///
    /// Exact only when [`Stack::new`] was called with an odd size (full
    /// tracking). Otherwise only the bottom [`STACK_USAGE_SAMPLE`] words are
    /// checked, so the result is an upper bound close to `size()`.
    pub fn get_used_size(&self) -> usize {
        let cap = self.size();
        // count how many words at the bottom of the stack still hold the
        // sentinel, i.e. are untouched. The loop is bounded by `cap` so that it
        // can never read past the top of the stack (the sentinel is only
        // written to at most `cap` words by `Stack::new`).
        let mut free: usize = 0;
        unsafe {
            let mut ptr = self.mem.buf.bottom.cast::<usize>();
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
        self.mem.buf.len() / std::mem::size_of::<usize>()
    }

    /// Point to the high end of the allocated stack
    pub fn end(&self) -> *mut usize {
        let offset = self.offset();
        unsafe { (self.mem.buf.top.cast::<usize>()).offset(0 - offset as isize) }
    }

    /// Point to the low end of the allocated stack
    pub fn begin(&self) -> *mut usize {
        self.mem.buf.bottom.cast::<usize>()
    }

    /// Reserve a block for `T` on this stack.
    ///
    /// The returned token must be initialized with
    /// [`StackBoxUninit::init`]; dropping it uninitialized releases the block.
    #[must_use]
    pub fn alloc_uninit_box<T>(&mut self) -> StackBoxUninit<T> {
        StackBox::<T>::new_uninit(self)
    }

    // current bump offset in words
    fn offset(&self) -> usize {
        self.mem.offset.get()
    }

    // the offset can be restored from a guard
    fn offset_cell(&self) -> &Cell<usize> {
        &self.mem.offset
    }

    /// Mark the stack as abandoned mid-run.
    ///
    /// A stack overflow yields out of the running frames without unwinding
    /// them, so the functor block allocated by `init_code` is never freed.
    /// The bump offset cannot be trusted after that, so the LIFO check is
    /// skipped when the stack's blocks are dropped.
    pub(crate) fn abandon(&self) {
        self.mem.abandoned.set(true);
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let offset = self.offset();
        write!(f, "Stack<{:?}, Offset={}>", self.mem.buf, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_address_aligns_to_the_real_address() {
        const ALIGN: usize = 8192;
        // `mmap` only guarantees page alignment, so a stack top can look like
        // this even when the data requires a bigger alignment.
        let old_end = 0x1000_1000;
        assert_ne!(old_end % ALIGN, 0);

        let data_addr = data_address(old_end, 100, ALIGN).unwrap();

        assert_eq!(data_addr % ALIGN, 0);
        assert!(data_addr + 100 <= old_end);
        assert_eq!((data_addr - HEADER_BYTES) % HEADER_ALIGN, 0);
    }

    #[test]
    fn data_address_rejects_blocks_below_address_zero() {
        assert!(data_address(0, 1, HEADER_ALIGN).is_none());
    }

    #[test]
    fn reserve_keeps_the_initial_frame_headroom_free() {
        let stack = Stack::new(17);
        let (_, _, end_offset) = StackBox::<u64>::reserve(&stack, 0, INITIAL_FRAME_RESERVE);

        // the headroom must not be part of the reserved block
        assert_eq!(stack.offset(), end_offset);
        assert!(end_offset + INITIAL_FRAME_RESERVE <= stack.size());
    }

    #[test]
    #[should_panic(expected = "allocation exceeds stack")]
    fn reserve_rejects_blocks_without_initial_frame_headroom() {
        let stack = Stack::new(17);
        // ask for more free space than the stack has
        StackBox::<u64>::reserve(&stack, 0, stack.size());
    }

    #[repr(C, align(8192))]
    struct OverAligned([u8; 8]);

    #[test]
    fn stack_box_aligns_over_aligned_data() {
        let mut stack = Stack::new(0x2000);
        let boxed = stack
            .alloc_uninit_box::<OverAligned>()
            .init(OverAligned([1; 8]));

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
        let stack = Stack::new(0x100);
        let offset = stack.offset();

        let boxed = StackBox::<PanicOnDrop>::new_uninit(&stack).init(PanicOnDrop);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(boxed)));
        assert!(result.is_err());
        assert_eq!(stack.offset(), offset);
    }

    #[test]
    fn stack_box_uninit_drop_releases_the_block() {
        let mut stack = Stack::new(0x100);
        let offset = stack.offset();

        // dropping a token that was never initialized must not leak the block
        drop(stack.alloc_uninit_box::<u64>());

        assert_eq!(stack.offset(), offset);
    }

    #[test]
    fn stack_box_frees_stack_when_drop_panics() {
        let mut stack = Stack::new(0x100);

        let boxed = stack.alloc_uninit_box::<PanicOnDrop>().init(PanicOnDrop);

        // the box's handle is the last one, so dropping it frees the mapping
        drop(stack);

        let deallocations_before = STACK_DEALLOCS.with(std::cell::Cell::get);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(boxed)));
        assert!(result.is_err());

        let deallocations_after = STACK_DEALLOCS.with(std::cell::Cell::get);
        assert_eq!(deallocations_after, deallocations_before + 1);
    }

    #[test]
    fn stack_drop_frees_the_mapping() {
        let deallocations_before = STACK_DEALLOCS.with(std::cell::Cell::get);

        // a stack that never gets boxed must not leak its mapping
        drop(Stack::new(0x100));

        let deallocations_after = STACK_DEALLOCS.with(std::cell::Cell::get);
        assert_eq!(deallocations_after, deallocations_before + 1);
    }

    #[test]
    fn stack_box_multiple_boxes_free_once() {
        let mut stack = Stack::new(0x100);

        let first = stack.alloc_uninit_box::<u64>().init(1);
        let second = stack.alloc_uninit_box::<u64>().init(2);

        let deallocations_before = STACK_DEALLOCS.with(std::cell::Cell::get);
        drop(second);
        drop(first);
        drop(stack);

        let deallocations_after = STACK_DEALLOCS.with(std::cell::Cell::get);
        assert_eq!(deallocations_after, deallocations_before + 1);
    }

    // The LIFO check is a `debug_assert`, so this test only exists in debug
    // builds.
    #[cfg(debug_assertions)]
    #[test]
    fn stack_box_detects_non_lifo_drop() {
        let stack = Stack::new(0x100);

        let first = StackBox::<u64>::new_uninit(&stack).init(1);
        let second = StackBox::<u64>::new_uninit(&stack).init(2);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(first)));
        assert!(
            result.is_err(),
            "freeing a block that is not the most recent allocation must trip the LIFO check"
        );

        drop(second);
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
            let stack = self.stack.clone();
            let out = self.out;

            // Same-stack allocation from a destructor: it must land below the
            // box being dropped, which is still reserved at this point.
            let nested = StackBox::<u64>::new_uninit(&stack).init(0xAA);
            unsafe { (*out).set(Some(nested.as_ptr() as usize)) };
        }
    }

    #[test]
    fn stack_box_stays_reserved_while_dropping() {
        let mut stack = Stack::new(0x100);
        let uninit = stack.alloc_uninit_box::<AllocOnDrop>();
        let self_addr = uninit.as_ptr() as usize;
        let self_end = self_addr + std::mem::size_of::<AllocOnDrop>();
        let nested_addr = std::cell::Cell::new(None);

        let boxed = uninit.init(AllocOnDrop {
            stack,
            out: &nested_addr,
        });
        drop(boxed);

        let nested_start = nested_addr.get().expect("destructor did not run");
        let nested_end = nested_start + std::mem::size_of::<u64>();
        assert!(
            nested_end <= self_addr || nested_start >= self_end,
            "nested allocation {nested_start:#x}..{nested_end:#x} overlaps \
             the box being dropped {self_addr:#x}..{self_end:#x}"
        );
    }
}
