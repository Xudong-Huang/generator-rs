pub mod Windows {
    pub mod Win32 {
        #[inline]
        pub unsafe fn AddVectoredExceptionHandler(
            first: u32,
            handler: PVECTORED_EXCEPTION_HANDLER,
        ) -> *mut core::ffi::c_void {
            windows_core::link!("kernel32.dll" "system" fn AddVectoredExceptionHandler(first : u32, handler : PVECTORED_EXCEPTION_HANDLER) -> *mut core::ffi::c_void);
            unsafe { AddVectoredExceptionHandler(first, handler) }
        }
        #[inline]
        pub unsafe fn GetStdHandle(nstdhandle: u32) -> HANDLE {
            windows_core::link!("kernel32.dll" "system" fn GetStdHandle(nstdhandle : u32) -> HANDLE);
            unsafe { GetStdHandle(nstdhandle) }
        }
        #[inline]
        pub unsafe fn GetSystemInfo(lpsysteminfo: *mut SYSTEM_INFO) {
            windows_core::link!("kernel32.dll" "system" fn GetSystemInfo(lpsysteminfo : *mut SYSTEM_INFO));
            unsafe { GetSystemInfo(lpsysteminfo as _) }
        }
        #[inline]
        pub unsafe fn VirtualAlloc(
            lpaddress: Option<*const core::ffi::c_void>,
            dwsize: usize,
            flallocationtype: u32,
            flprotect: u32,
        ) -> *mut core::ffi::c_void {
            windows_core::link!("kernel32.dll" "system" fn VirtualAlloc(lpaddress : *const core::ffi::c_void, dwsize : usize, flallocationtype : u32, flprotect : u32) -> *mut core::ffi::c_void);
            unsafe {
                VirtualAlloc(
                    lpaddress.unwrap_or(core::mem::zeroed()) as _,
                    dwsize,
                    flallocationtype,
                    flprotect,
                )
            }
        }
        #[inline]
        pub unsafe fn VirtualFree(
            lpaddress: *mut core::ffi::c_void,
            dwsize: usize,
            dwfreetype: u32,
        ) -> windows_core::BOOL {
            windows_core::link!("kernel32.dll" "system" fn VirtualFree(lpaddress : *mut core::ffi::c_void, dwsize : usize, dwfreetype : u32) -> windows_core::BOOL);
            unsafe { VirtualFree(lpaddress as _, dwsize, dwfreetype) }
        }
        #[inline]
        pub unsafe fn VirtualProtect(
            lpaddress: *const core::ffi::c_void,
            dwsize: usize,
            flnewprotect: u32,
            lpfloldprotect: *mut u32,
        ) -> windows_core::BOOL {
            windows_core::link!("kernel32.dll" "system" fn VirtualProtect(lpaddress : *const core::ffi::c_void, dwsize : usize, flnewprotect : u32, lpfloldprotect : *mut u32) -> windows_core::BOOL);
            unsafe { VirtualProtect(lpaddress, dwsize, flnewprotect, lpfloldprotect as _) }
        }
        #[inline]
        pub unsafe fn WriteFile(
            hfile: HANDLE,
            lpbuffer: Option<*const core::ffi::c_void>,
            nnumberofbytestowrite: u32,
            lpnumberofbyteswritten: Option<*mut u32>,
            lpoverlapped: Option<*mut OVERLAPPED>,
        ) -> windows_core::BOOL {
            windows_core::link!("kernel32.dll" "system" fn WriteFile(hfile : HANDLE, lpbuffer : *const core::ffi::c_void, nnumberofbytestowrite : u32, lpnumberofbyteswritten : *mut u32, lpoverlapped : *mut OVERLAPPED) -> windows_core::BOOL);
            unsafe {
                WriteFile(
                    hfile,
                    lpbuffer.unwrap_or(core::mem::zeroed()) as _,
                    nnumberofbytestowrite,
                    lpnumberofbyteswritten.unwrap_or(core::mem::zeroed()) as _,
                    lpoverlapped.unwrap_or(core::mem::zeroed()) as _,
                )
            }
        }
        #[repr(C, align(16))]
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86", target_arch = "x86_64"))]
        #[derive(Clone, Copy)]
        pub struct ARM64_NT_CONTEXT {
            pub ContextFlags: u32,
            pub Cpsr: u32,
            pub Anonymous: ARM64_NT_CONTEXT_0,
            pub Sp: u64,
            pub Pc: u64,
            pub V: [ARM64_NT_NEON128; 32],
            pub Fpcr: u32,
            pub Fpsr: u32,
            pub Bcr: [u32; 8],
            pub Bvr: [u64; 8],
            pub Wcr: [u32; 2],
            pub Wvr: [u64; 2],
        }
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86", target_arch = "x86_64"))]
        impl Default for ARM64_NT_CONTEXT {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86", target_arch = "x86_64"))]
        #[derive(Clone, Copy)]
        pub union ARM64_NT_CONTEXT_0 {
            pub Anonymous: ARM64_NT_CONTEXT_0_0,
            pub X: [u64; 31],
        }
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86", target_arch = "x86_64"))]
        impl Default for ARM64_NT_CONTEXT_0 {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86", target_arch = "x86_64"))]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct ARM64_NT_CONTEXT_0_0 {
            pub X0: u64,
            pub X1: u64,
            pub X2: u64,
            pub X3: u64,
            pub X4: u64,
            pub X5: u64,
            pub X6: u64,
            pub X7: u64,
            pub X8: u64,
            pub X9: u64,
            pub X10: u64,
            pub X11: u64,
            pub X12: u64,
            pub X13: u64,
            pub X14: u64,
            pub X15: u64,
            pub X16: u64,
            pub X17: u64,
            pub X18: u64,
            pub X19: u64,
            pub X20: u64,
            pub X21: u64,
            pub X22: u64,
            pub X23: u64,
            pub X24: u64,
            pub X25: u64,
            pub X26: u64,
            pub X27: u64,
            pub X28: u64,
            pub Fp: u64,
            pub Lr: u64,
        }
        #[repr(C, align(16))]
        #[cfg(target_arch = "aarch64")]
        #[derive(Clone, Copy)]
        pub struct ARM64_NT_CONTEXT {
            pub ContextFlags: u32,
            pub Cpsr: u32,
            pub Anonymous: ARM64_NT_CONTEXT_0,
            pub Sp: u64,
            pub Pc: u64,
            pub V: [NEON128; 32],
            pub Fpcr: u32,
            pub Fpsr: u32,
            pub Bcr: [u32; 8],
            pub Bvr: [u64; 8],
            pub Wcr: [u32; 2],
            pub Wvr: [u64; 2],
        }
        #[cfg(target_arch = "aarch64")]
        impl Default for ARM64_NT_CONTEXT {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(target_arch = "aarch64")]
        #[derive(Clone, Copy)]
        pub union ARM64_NT_CONTEXT_0 {
            pub Anonymous: ARM64_NT_CONTEXT_0_0,
            pub X: [u64; 31],
        }
        #[cfg(target_arch = "aarch64")]
        impl Default for ARM64_NT_CONTEXT_0 {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(target_arch = "aarch64")]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct ARM64_NT_CONTEXT_0_0 {
            pub X0: u64,
            pub X1: u64,
            pub X2: u64,
            pub X3: u64,
            pub X4: u64,
            pub X5: u64,
            pub X6: u64,
            pub X7: u64,
            pub X8: u64,
            pub X9: u64,
            pub X10: u64,
            pub X11: u64,
            pub X12: u64,
            pub X13: u64,
            pub X14: u64,
            pub X15: u64,
            pub X16: u64,
            pub X17: u64,
            pub X18: u64,
            pub X19: u64,
            pub X20: u64,
            pub X21: u64,
            pub X22: u64,
            pub X23: u64,
            pub X24: u64,
            pub X25: u64,
            pub X26: u64,
            pub X27: u64,
            pub X28: u64,
            pub Fp: u64,
            pub Lr: u64,
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub union ARM64_NT_NEON128 {
            pub Anonymous: ARM64_NT_NEON128_0,
            pub D: [f64; 2],
            pub S: [f32; 4],
            pub H: [u16; 8],
            pub B: [u8; 16],
        }
        impl Default for ARM64_NT_NEON128 {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct ARM64_NT_NEON128_0 {
            pub Low: u64,
            pub High: i64,
        }
        #[repr(C)]
        #[cfg(target_arch = "x86")]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct CONTEXT {
            pub ContextFlags: u32,
            pub Dr0: u32,
            pub Dr1: u32,
            pub Dr2: u32,
            pub Dr3: u32,
            pub Dr6: u32,
            pub Dr7: u32,
            pub FloatSave: FLOATING_SAVE_AREA,
            pub SegGs: u32,
            pub SegFs: u32,
            pub SegEs: u32,
            pub SegDs: u32,
            pub Edi: u32,
            pub Esi: u32,
            pub Ebx: u32,
            pub Edx: u32,
            pub Ecx: u32,
            pub Eax: u32,
            pub Ebp: u32,
            pub Eip: u32,
            pub SegCs: u32,
            pub EFlags: u32,
            pub Esp: u32,
            pub SegSs: u32,
            pub ExtendedRegisters: [u8; 512],
        }
        #[cfg(target_arch = "x86")]
        impl Default for CONTEXT {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
        #[derive(Clone, Copy)]
        pub struct CONTEXT {
            pub P1Home: u64,
            pub P2Home: u64,
            pub P3Home: u64,
            pub P4Home: u64,
            pub P5Home: u64,
            pub P6Home: u64,
            pub ContextFlags: u32,
            pub MxCsr: u32,
            pub SegCs: u16,
            pub SegDs: u16,
            pub SegEs: u16,
            pub SegFs: u16,
            pub SegGs: u16,
            pub SegSs: u16,
            pub EFlags: u32,
            pub Dr0: u64,
            pub Dr1: u64,
            pub Dr2: u64,
            pub Dr3: u64,
            pub Dr6: u64,
            pub Dr7: u64,
            pub Rax: u64,
            pub Rcx: u64,
            pub Rdx: u64,
            pub Rbx: u64,
            pub Rsp: u64,
            pub Rbp: u64,
            pub Rsi: u64,
            pub Rdi: u64,
            pub R8: u64,
            pub R9: u64,
            pub R10: u64,
            pub R11: u64,
            pub R12: u64,
            pub R13: u64,
            pub R14: u64,
            pub R15: u64,
            pub Rip: u64,
            pub Anonymous: CONTEXT_0,
            pub VectorRegister: [M128A; 26],
            pub VectorControl: u64,
            pub DebugControl: u64,
            pub LastBranchToRip: u64,
            pub LastBranchFromRip: u64,
            pub LastExceptionToRip: u64,
            pub LastExceptionFromRip: u64,
        }
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
        impl Default for CONTEXT {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
        #[derive(Clone, Copy)]
        pub union CONTEXT_0 {
            pub FltSave: XMM_SAVE_AREA32,
            pub Anonymous: CONTEXT_0_0,
        }
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
        impl Default for CONTEXT_0 {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct CONTEXT_0_0 {
            pub Header: [M128A; 2],
            pub Legacy: [M128A; 8],
            pub Xmm0: M128A,
            pub Xmm1: M128A,
            pub Xmm2: M128A,
            pub Xmm3: M128A,
            pub Xmm4: M128A,
            pub Xmm5: M128A,
            pub Xmm6: M128A,
            pub Xmm7: M128A,
            pub Xmm8: M128A,
            pub Xmm9: M128A,
            pub Xmm10: M128A,
            pub Xmm11: M128A,
            pub Xmm12: M128A,
            pub Xmm13: M128A,
            pub Xmm14: M128A,
            pub Xmm15: M128A,
        }
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
        impl Default for CONTEXT_0_0 {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[cfg(target_arch = "aarch64")]
        pub type CONTEXT = ARM64_NT_CONTEXT;
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct EXCEPTION_POINTERS {
            pub ExceptionRecord: PEXCEPTION_RECORD,
            pub ContextRecord: PCONTEXT,
        }
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct EXCEPTION_RECORD {
            pub ExceptionCode: u32,
            pub ExceptionFlags: u32,
            pub ExceptionRecord: *mut Self,
            pub ExceptionAddress: *mut core::ffi::c_void,
            pub NumberParameters: u32,
            pub ExceptionInformation: [usize; 15],
        }
        impl Default for EXCEPTION_RECORD {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(target_arch = "x86")]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct FLOATING_SAVE_AREA {
            pub ControlWord: u32,
            pub StatusWord: u32,
            pub TagWord: u32,
            pub ErrorOffset: u32,
            pub ErrorSelector: u32,
            pub DataOffset: u32,
            pub DataSelector: u32,
            pub RegisterArea: [u8; 80],
            pub Spare0: u32,
        }
        #[cfg(target_arch = "x86")]
        impl Default for FLOATING_SAVE_AREA {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
        pub struct HANDLE(pub *mut core::ffi::c_void);
        #[repr(C, align(16))]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct M128A {
            pub Low: u64,
            pub High: i64,
        }
        #[cfg(target_arch = "aarch64")]
        pub type NEON128 = ARM64_NT_NEON128;
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct OVERLAPPED {
            pub Internal: usize,
            pub InternalHigh: usize,
            pub Anonymous: OVERLAPPED_0,
            pub hEvent: HANDLE,
        }
        impl Default for OVERLAPPED {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub union OVERLAPPED_0 {
            pub Anonymous: OVERLAPPED_0_0,
            pub Pointer: *mut core::ffi::c_void,
        }
        impl Default for OVERLAPPED_0 {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct OVERLAPPED_0_0 {
            pub Offset: u32,
            pub OffsetHigh: u32,
        }
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86", target_arch = "x86_64"))]
        pub type PCONTEXT = *mut CONTEXT;
        #[cfg(target_arch = "aarch64")]
        pub type PCONTEXT = *mut ARM64_NT_CONTEXT;
        pub type PEXCEPTION_RECORD = *mut EXCEPTION_RECORD;
        pub type PVECTORED_EXCEPTION_HANDLER =
            Option<unsafe extern "system" fn(exceptioninfo: *mut EXCEPTION_POINTERS) -> i32>;
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct SYSTEM_INFO {
            pub Anonymous: SYSTEM_INFO_0,
            pub dwPageSize: u32,
            pub lpMinimumApplicationAddress: *mut core::ffi::c_void,
            pub lpMaximumApplicationAddress: *mut core::ffi::c_void,
            pub dwActiveProcessorMask: usize,
            pub dwNumberOfProcessors: u32,
            pub dwProcessorType: u32,
            pub dwAllocationGranularity: u32,
            pub wProcessorLevel: u16,
            pub wProcessorRevision: u16,
        }
        impl Default for SYSTEM_INFO {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub union SYSTEM_INFO_0 {
            pub dwOemId: u32,
            pub Anonymous: SYSTEM_INFO_0_0,
        }
        impl Default for SYSTEM_INFO_0 {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        pub struct SYSTEM_INFO_0_0 {
            pub wProcessorArchitecture: u16,
            pub wReserved: u16,
        }
        #[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
        pub type XMM_SAVE_AREA32 = XSAVE_FORMAT;
        #[repr(C)]
        #[cfg(target_arch = "x86")]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct XSAVE_FORMAT {
            pub ControlWord: u16,
            pub StatusWord: u16,
            pub TagWord: u8,
            pub Reserved1: u8,
            pub ErrorOpcode: u16,
            pub ErrorOffset: u32,
            pub ErrorSelector: u16,
            pub Reserved2: u16,
            pub DataOffset: u32,
            pub DataSelector: u16,
            pub Reserved3: u16,
            pub MxCsr: u32,
            pub MxCsr_Mask: u32,
            pub FloatRegisters: [M128A; 8],
            pub XmmRegisters: [M128A; 8],
            pub Reserved4: [u8; 224],
        }
        #[cfg(target_arch = "x86")]
        impl Default for XSAVE_FORMAT {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
        #[repr(C)]
        #[cfg(any(
            target_arch = "aarch64",
            target_arch = "arm64ec",
            target_arch = "x86_64"
        ))]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct XSAVE_FORMAT {
            pub ControlWord: u16,
            pub StatusWord: u16,
            pub TagWord: u8,
            pub Reserved1: u8,
            pub ErrorOpcode: u16,
            pub ErrorOffset: u32,
            pub ErrorSelector: u16,
            pub Reserved2: u16,
            pub DataOffset: u32,
            pub DataSelector: u16,
            pub Reserved3: u16,
            pub MxCsr: u32,
            pub MxCsr_Mask: u32,
            pub FloatRegisters: [M128A; 8],
            pub XmmRegisters: [M128A; 16],
            pub Reserved4: [u8; 96],
        }
        #[cfg(any(
            target_arch = "aarch64",
            target_arch = "arm64ec",
            target_arch = "x86_64"
        ))]
        impl Default for XSAVE_FORMAT {
            fn default() -> Self {
                unsafe { core::mem::zeroed() }
            }
        }
    }
}
