.text
.globl prefetch
.type prefetch,@function
.align 16
prefetch:
    mov eax, [esp + 4]
    prefetcht2 [eax]
    ret
.size prefetch,.-prefetch

.text
.globl bootstrap_green_task
.type bootstrap_green_task,@function
.align 16
bootstrap_green_task:
    /* The i386 ABI passes the arguments on the stack, so unlike x86_64 we
       have to build the frame of the init fn here. It expects esp + 4 to be
       16 byte aligned on entry, that is esp % 16 == 12. */
    and esp, -16                 /* esp % 16 == 0 */
    lea esp, [esp - 24]
    mov [esp + 8], ebx           /* first argument */
    mov [esp + 12], esi          /* second argument */
    mov dword ptr [esp + 4], 0   /* the init fn never returns, no return address */
    mov [esp], edi               /* the ret below jumps here */
    ret
.size bootstrap_green_task,.-bootstrap_green_task

.text
.globl swap_registers
.type swap_registers,@function
.align 16
swap_registers:
    /* read both arguments before esp is switched over */
    mov eax, [esp + 4]  /* out_regs */
    mov edx, [esp + 8]  /* in_regs */

    mov [eax + 0*4], ebx
    mov [eax + 1*4], esp
    mov [eax + 2*4], ebp
    mov [eax + 3*4], esi
    mov [eax + 4*4], edi

    mov ebx, [edx + 0*4]
    mov esp, [edx + 1*4]
    mov ebp, [edx + 2*4]
    mov esi, [edx + 3*4]
    mov edi, [edx + 4*4]

    /* the saved esp points at the return address, the caller pops the
       arguments itself */
    pop ecx
    jmp ecx
.size swap_registers,.-swap_registers

/* Mark that we don't need executable stack. */
.section .note.GNU-stack,"",%progbits
