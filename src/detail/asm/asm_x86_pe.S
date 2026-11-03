.file	"asm_x86_pe.S"

/* 32 bit COFF decorates the cdecl symbols with a leading underscore */
.text
.p2align 4,,15
.globl	_prefetch_asm
.def	_prefetch_asm;	.scl	2;	.type	32;	.endef
_prefetch_asm:
    mov eax, [esp + 4]
    prefetcht2 [eax]
    ret

.text
.p2align 4,,15
.globl	_bootstrap_green_task
.def	_bootstrap_green_task;	.scl	2;	.type	32;	.endef
_bootstrap_green_task:
    /* The i386 ABI passes the arguments on the stack, so we have to build
       the frame of the init fn here. It expects esp + 4 to be 16 byte
       aligned on entry, that is esp % 16 == 12. */
    and esp, -16                 /* esp % 16 == 0 */
    lea esp, [esp - 24]
    mov [esp + 8], ebx           /* first argument */
    mov [esp + 12], esi          /* second argument */
    mov dword ptr [esp + 4], 0   /* the init fn never returns, no return address */
    mov [esp], edi               /* the ret below jumps here */
    ret

.text
.p2align 4,,15
.globl	_swap_registers
.def	_swap_registers;	.scl	2;	.type	32;	.endef
_swap_registers:
    /* read both arguments before esp is switched over */
    mov eax, [esp + 4]  /* out_regs */
    mov edx, [esp + 8]  /* in_regs */

    mov [eax + 0*4], ebx
    mov [eax + 1*4], esp
    mov [eax + 2*4], ebp
    mov [eax + 3*4], esi
    mov [eax + 4*4], edi

    /* save the thread information block fields that describe the stack */
    /* current SEH chain */
    mov ecx, fs:[0x00]
    mov [eax + 5*4], ecx
    /* current stack base */
    mov ecx, fs:[0x04]
    mov [eax + 6*4], ecx
    /* current stack limit */
    mov ecx, fs:[0x08]
    mov [eax + 7*4], ecx
    /* current deallocation stack */
    mov ecx, fs:[0x0E0C]
    mov [eax + 8*4], ecx

    mov ebx, [edx + 0*4]
    mov esp, [edx + 1*4]
    mov ebp, [edx + 2*4]
    mov esi, [edx + 3*4]
    mov edi, [edx + 4*4]

    /* restore deallocation stack */
    mov ecx, [edx + 8*4]
    mov fs:[0x0E0C], ecx
    /* restore stack limit */
    mov ecx, [edx + 7*4]
    mov fs:[0x08], ecx
    /* restore stack base */
    mov ecx, [edx + 6*4]
    mov fs:[0x04], ecx
    /* restore SEH chain */
    mov ecx, [edx + 5*4]
    mov fs:[0x00], ecx

    /* the saved esp points at the return address, the caller pops the
       arguments itself */
    pop ecx
    jmp ecx
