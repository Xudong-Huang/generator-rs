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

// must align with StackBoxHeader
const ALIGN: usize = std::mem::size_of::<StackBoxHeader>();
const HEADER_SIZE: usize = std::mem::size_of::<StackBoxHeader>() / std::mem::size_of::<usize>();

struct StackBoxHeader {
    // track the stack
    stack: Stack,
    // track how big the data is (in usize)
    data_size: usize,
    // non zero dealloc the stack
    need_drop: usize,
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
        // alloc the data
        let layout = std::alloc::Layout::new::<T>();
        let align = std::cmp::max(layout.align(), ALIGN);
        let size = ((layout.size() + align - 1) & !(align - 1)) / std::mem::size_of::<usize>();
        let u_align = align / std::mem::size_of::<usize>();
        let pad_size = u_align - (*offset + size) % u_align;
        let data_size = size + pad_size;

        // `Stack::end()` is `top - offset`, so the bump cursor has to stay
        // within the stack: past `stack.size()` the header write below, and the
        // caller's `init()` after it, both land under `buf.bottom` -- through
        // the guard page and into whatever mapping happens to sit there.
        let needed = *offset + data_size + HEADER_SIZE;
        if needed > stack.size() {
            stack_too_small(
                stack,
                need_drop,
                *offset,
                needed,
                std::any::type_name::<T>(),
            );
        }

        *offset += data_size;
        let ptr = unsafe { ptr::NonNull::new_unchecked(stack.end() as *mut T) };

        // init the header
        *offset += HEADER_SIZE;
        unsafe {
            let mut header = ptr::NonNull::new_unchecked(stack.end() as *mut StackBoxHeader);
            let header = header.as_mut();
            header.data_size = data_size;
            header.need_drop = need_drop;
            header.stack = stack.shadow_clone();
            MaybeUninit::new(StackBox { ptr })
        }
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

/// The bump allocator would have run off the end of the generator stack.
///
/// This is not a runtime stack overflow: it means the stack was never big
/// enough to hold the generator's own bookkeeping plus the closure it has to
/// carry, so it is a caller error and reported as one.
#[cold]
#[inline(never)]
fn stack_too_small(stack: &Stack, need_drop: usize, offset: usize, needed: usize, what: &str) -> ! {
    let word = std::mem::size_of::<usize>();
    let cap = stack.size();

    // A pristine stack (offset still at its initial 1) has handed out nothing,
    // so there is no `StackBox` left to release the mapping while we unwind and
    // we have to do it here. Otherwise the owning box frees it on the way out.
    if need_drop != 0 && offset == 1 {
        stack.drop_stack();
    }

    panic!(
        "generator stack too small to hold {what}: needs {} bytes but the stack is {} bytes; \
         pass a bigger size to Gn::new_opt/new_scoped_opt",
        needed * word,
        cap * word,
    );
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

impl<T> Drop for StackBox<T> {
    fn drop(&mut self) {
        let header = self.get_header();
        unsafe {
            *header.stack.get_offset() -= header.data_size + HEADER_SIZE;
            ptr::drop_in_place(self.ptr.as_ptr());
            if header.need_drop != 0 {
                header.stack.drop_stack();
            }
        }
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

        unsafe {
            let buf = stk.buf.bottom as *mut usize;
            ptr::write_bytes(buf, 0xEE, count);
        }

        // init the stack box usage
        let offset = stk.get_offset();
        unsafe { *offset = 1 };

        stk
    }

    /// get used stack size
    pub fn get_used_size(&self) -> usize {
        let mut offset: usize = 0;
        unsafe {
            let mut magic: usize = 0xEE;
            ptr::write_bytes(&mut magic, 0xEE, 1);
            let mut ptr = self.buf.bottom as *mut usize;
            while *ptr == magic {
                offset += 1;
                ptr = ptr.offset(1);
            }
        }
        let cap = self.size();
        cap - offset
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
    use std::panic::{self, AssertUnwindSafe};
    use std::process::Command;

    /// Set by the parent test on the child process. Without it the child test
    /// body is a no-op, so that a plain `cargo test -- --ignored` cannot take
    /// the whole harness down with a SIGSEGV.
    const CHILD_MARKER: &str = "GENERATOR_BOF_001_CHILD";

    /// The child test's path, as `--exact` wants it.
    const CHILD_TEST: &str = "stack::tests::bof_001_child";

    /// What `stack_too_small` is expected to say.
    const EXPECTED_PANIC: &str = "generator stack too small";

    /// Exit code the child uses when the allocator handed out memory it should
    /// have refused. The out-of-bounds write has already happened by then.
    const EXIT_ALLOWED: i32 = 42;

    /// Exit code the child uses when it was refused, but by some other panic.
    const EXIT_WRONG_PANIC: i32 = 43;

    /// Exactly 1MiB of usable stack: `SysStack::allocate` rounds the request
    /// down to a page boundary and then adds one page back, so any page size up
    /// to 1MiB lands on 1MiB here.
    const STACK_WORDS: usize = (1 << 20) / std::mem::size_of::<usize>();

    /// 1MiB plus 512 bytes. The 512 byte excess is what makes the reproducer
    /// deterministic on unbounded code: the allocation overshoots `buf.bottom`
    /// by a few hundred bytes, which is inside the one page guard, rather than
    /// by a page or more into whatever mapping sits below it.
    type Payload = [u8; (1 << 20) + 512];

    /// BOF-001: `StackBox::new_uninit` is a bump allocator, and without a bound
    /// on its cursor a payload larger than the stack makes `Stack::end()` point
    /// below the mapping, so the `StackBoxHeader` write at that address goes
    /// out of bounds.
    ///
    /// The allocation runs in a child process: if the bound is ever removed
    /// again the write faults on the guard page, and this way that shows up as
    /// a failing test rather than as a dead test harness.
    #[test]
    fn bof_001_new_uninit_must_not_allocate_past_the_end_of_the_stack() {
        let exe = std::env::current_exe().expect("cannot locate the test binary");
        let out = Command::new(&exe)
            .args([
                "--exact",
                CHILD_TEST,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_MARKER, "1")
            .output()
            .expect("cannot run the BOF-001 child process");

        if out.status.success() {
            // Refused with the expected panic, before touching any memory.
            return;
        }

        let stack_bytes = STACK_WORDS * std::mem::size_of::<usize>();
        let what = match out.status.code() {
            Some(EXIT_ALLOWED) => format!(
                "allocating {} bytes on a {stack_bytes} byte stack was allowed and escaped the mapping",
                std::mem::size_of::<Payload>()
            ),
            Some(EXIT_WRONG_PANIC) => "the allocation was refused, but not by the bounds check".to_string(),
            _ => format!(
                "the child died ({}), i.e. the StackBoxHeader write hit the guard page",
                out.status
            ),
        };

        panic!(
            "BOF-001: {what}\n--- child stdout ---\n{}\n--- child stderr ---\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }

    /// The other half of `bof_001_new_uninit_must_not_allocate_past_the_end_of_the_stack`.
    /// Never run this directly: on unbounded code it crashes the process.
    #[test]
    #[ignore = "BOF-001 reproducer, run as a child process by the test above"]
    fn bof_001_child() {
        if std::env::var_os(CHILD_MARKER).is_none() {
            return;
        }

        let mut stack = Stack::new(STACK_WORDS);
        let begin = stack.begin();
        let top = unsafe { begin.add(stack.size()) };

        // Precondition: the payload genuinely does not fit in the stack.
        assert!(std::mem::size_of::<Payload>() > stack.size() * std::mem::size_of::<usize>());

        // A bounded allocator refuses this before touching any memory. An
        // unbounded one writes the StackBoxHeader below `buf.bottom` instead,
        // and on a guard page hit we never come back from this call.
        let alloc = panic::catch_unwind(AssertUnwindSafe(|| stack.alloc_uninit_box::<Payload>()));

        let b = match alloc {
            Err(payload) => {
                let msg = panic_message(&*payload);
                if !msg.contains(EXPECTED_PANIC) {
                    println!("refused, but by an unexpected panic: {msg}");
                    std::process::exit(EXIT_WRONG_PANIC);
                }
                return;
            }
            Ok(b) => b,
        };

        // The allocation went through, which the precondition says it must not
        // have. Report where it landed; `StackBox::drop` would read the header
        // back from out of bounds, so leak the box instead.
        let b = unsafe { b.assume_init() };
        let ptr = b.as_ptr() as *mut usize;
        std::mem::forget(b);

        println!("allocation landed at {ptr:?}, stack is [{begin:?}, {top:?})");
        std::process::exit(EXIT_ALLOWED);
    }

    /// The bound must not be off by one: an allocation that does fit has to
    /// keep working and has to land inside the mapping.
    #[test]
    fn alloc_uninit_box_stays_inside_the_stack() {
        let mut stack = Stack::new(STACK_WORDS);
        let begin = stack.begin();
        let top = unsafe { begin.add(stack.size()) };

        let b = unsafe { stack.alloc_uninit_box::<[u8; 4096]>().assume_init() };
        let ptr = b.as_ptr() as *mut usize;
        // dropping the box also releases the stack, so exercise that path
        drop(b);

        assert!(
            ptr >= begin && ptr < top,
            "{ptr:?} is outside [{begin:?}, {top:?})"
        );
    }

    fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
        payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&'static str>().copied())
            .unwrap_or("<non string panic payload>")
    }
}
