//! # generator stack
//!
//!

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io;
use std::os::raw::c_void;
use std::ptr;

#[cfg(all(unix, target_arch = "x86_64"))]
#[path = "unix.rs"]
pub mod sys;

#[cfg(all(windows, target_arch = "x86_64"))]
#[path = "windows.rs"]
pub mod sys;

/// Error type returned by stack allocation methods.
#[derive(Debug)]
pub enum StackError {
    /// Contains the maximum amount of memory allowed to be allocated as stack space.
    ExceedsMaximumSize(usize),

    /// Returned if some kind of I/O error happens during allocation.
    IoError(io::Error),
}

impl Display for StackError {
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
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
    fn description(&self) -> &str {
        match *self {
            StackError::ExceedsMaximumSize(_) => "exceeds maximum stack size",
            StackError::IoError(ref e) => e.description(),
        }
    }
    fn cause(&self) -> Option<&dyn Error> {
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

        size = (size - 1) & !(page_size - 1);

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
    track: bool,
}

impl Stack {
    pub fn empty() -> Stack {
        Stack {
            buf: SysStack {
                top: ptr::null_mut(),
                bottom: ptr::null_mut(),
            },
            track: false,
        }
    }

    /// Allocate a new stack of `size`. If size = 0, this is a `dummy_stack`
    pub fn new(size: usize) -> Stack {
        let track = (size & 1) != 0;
        let mut bytes = size * std::mem::size_of::<usize>();
        // the minimal size
        let min_size = SysStack::min_size();

        if bytes < min_size {
            bytes = min_size;
        }

        let buf = SysStack::allocate(bytes, true).expect("failed to alloc sys stack");

        let stk = Stack { buf, track };

        // if size is not even we do the full foot print test
        let count = if track {
            stk.size()
        } else {
            // we only check the last few words
            8
        };

        unsafe {
            let buf = stk.buf.bottom() as *mut usize;
            ptr::write_bytes(buf, 0xEE, count);
        }
        stk
    }

    /// get used stack size
    pub fn get_used_size(&self) -> usize {
        let mut offset: usize = 0;
        unsafe {
            let mut magic: usize = 0xEE;
            ptr::write_bytes(&mut magic, 0xEE, 1);
            let mut ptr = self.buf.bottom() as *mut usize;
            while *ptr == magic {
                offset += 1;
                ptr = ptr.offset(1);
            }
        }
        let cap = self.size();
        if self.track {
            eprintln!("stack size={}, used={}", cap, cap - offset);
        }
        cap - offset
    }

    /// get the stack cap
    #[inline]
    pub fn size(&self) -> usize {
        self.buf.len() / std::mem::size_of::<usize>()
    }

    /// Point to the high end of the allocated stack
    pub fn end(&self) -> *mut usize {
        self.buf.top() as *mut _
    }

    /// Point to the low end of the allocated stack
    #[allow(dead_code)]
    pub fn begin(&self) -> *mut usize {
        self.buf.bottom() as *mut _
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        if self.buf.len() == 0 {
            return;
        }
        let page_size = sys::page_size();
        let guard = (self.buf.bottom() as usize - page_size) as *mut c_void;
        let size_with_guard = self.buf.len() + page_size;
        unsafe {
            sys::deallocate_stack(guard, size_with_guard);
        }
    }
}
