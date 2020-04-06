//! # generator stack
//!
//!

use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::mem::MaybeUninit;
use std::os::raw::c_void;
use std::ptr;

#[cfg(all(unix, target_arch = "x86_64"))]
#[path = "unix.rs"]
pub mod sys;

#[cfg(all(windows, target_arch = "x86_64"))]
#[path = "windows.rs"]
pub mod sys;

/// A pointer type for stack allocation.
pub struct StackBox<T> {
    ptr: ptr::NonNull<T>,
    // track the stack offset, saved on stack
    offset: *mut usize,
    // track how big the data is (in usize)
    size: usize,
}

const ALIGN: usize = std::mem::size_of::<usize>();

impl<T> StackBox<T> {
    /// create uninit stack box
    fn new_unint(stack: &mut Stack) -> MaybeUninit<Self> {
        let layout = std::alloc::Layout::new::<T>();
        let align = std::cmp::max(layout.align(), ALIGN);
        let size = ((layout.size() + align - 1) & !(align - 1)) / std::mem::size_of::<usize>();
        let offset = stack.get_offset();
        unsafe {
            *offset += size;
            let ptr = ptr::NonNull::new_unchecked(stack.end() as *mut T);
            std::mem::MaybeUninit::new(StackBox { ptr, offset, size })
        }
    }

    /// move data into the box
    pub unsafe fn init(&mut self, data: T) {
        ptr::write(self.ptr.as_mut(), data);
    }
}

pub struct Func {
    data: *mut (),
    size: usize,
    offset: *mut usize,
    is_called: bool,
    func: fn(*mut ()),
    drop: fn(*mut ()),
}

impl Func {
    pub fn call_once(mut self) {
        (self.func)(self.data);
        self.is_called = true;
    }
}

impl Drop for Func {
    fn drop(&mut self) {
        if !self.is_called {
            (self.drop)(self.data);
        }
        unsafe { *self.offset -= self.size }
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

    pub fn new_fn_once(stack: &mut Stack, data: F) -> Func {
        unsafe {
            let mut d = Self::new_unint(stack).assume_init();
            d.init(data);
            let f = Func {
                data: d.ptr.as_mut() as *mut _ as *mut (),
                size: d.size,
                offset: d.offset,
                is_called: false,
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
        unsafe { &*self.ptr.as_ref() }
    }
}

impl<T> std::ops::DerefMut for StackBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr.as_mut() }
    }
}

impl<T> Drop for StackBox<T> {
    fn drop(&mut self) {
        unsafe {
            *self.offset -= self.size;
            ptr::drop_in_place(self.ptr.as_mut());
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
                "Requested more than max size of {} bytes for a stack",
                size
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
        let add_shift = if protected { 1 } else { 0 };
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
pub struct Stack {
    buf: SysStack,
}

impl Stack {
    /// Allocate a new stack of `size`. If size = 0, this is a `dummy_stack`
    pub fn new(size: usize) -> StackBox<Stack> {
        let track = (size & 1) != 0;
        let mut bytes = size * std::mem::size_of::<usize>();
        // the minimal size
        let min_size = SysStack::min_size();

        if bytes < min_size {
            bytes = min_size;
        }

        let buf = SysStack::allocate(bytes, true).expect("failed to alloc sys stack");

        let mut stk = Stack { buf };

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
        unsafe { *offset = 2 };

        unsafe {
            let mut stack = stk.alloc_uninit_box::<Stack>().assume_init();
            stack.init(stk);
            stack
        }
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
    #[allow(dead_code)]
    pub fn begin(&self) -> *mut usize {
        self.buf.bottom as *mut _
    }

    /// alloc buffer on this stack
    pub fn alloc_uninit_box<T>(&mut self) -> MaybeUninit<StackBox<T>> {
        StackBox::<T>::new_unint(self)
    }

    // get offset
    fn get_offset(&self) -> *mut usize {
        unsafe { (self.buf.top as *mut usize).offset(-1) }
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let offset = self.get_offset();
        write!(f, "Statck<{:?}, Offset={}>", self.buf, unsafe { *offset })
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
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
}
