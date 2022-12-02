.text
.globl prefetch
.type prefetch,@function
.align 16
prefetch:
    prefetcht2 (%rdi)
    ret
.size prefetch,.-prefetch

.text
.globl bootstrap_green_task
.type bootstrap_green_task,@function
.align 16
bootstrap_green_task:
    mov %r12, %rdi     /* setup the function arg */
    mov %r13, %rsi     /* setup the function arg */
    and $-16, %rsp     /* align the stack pointer */
    mov %r14, (%rsp)   /* this is the new return adrress */
    ret
.size bootstrap_green_task,.-bootstrap_green_task

.text
.globl swap_registers
.type swap_registers,@function
.align 16
swap_registers:
    mov %rbx, (0*8)(%rdi)
    mov %rsp, (1*8)(%rdi)
    mov %rbp, (2*8)(%rdi)
    mov %r12, (4*8)(%rdi)
    mov %r13, (5*8)(%rdi)
    mov %r14, (6*8)(%rdi)
    mov %r15, (7*8)(%rdi)

    mov (0*8)(%rsi), %rbx
    mov (1*8)(%rsi), %rsp
    mov (2*8)(%rsi), %rbp
    mov (4*8)(%rsi), %r12
    mov (5*8)(%rsi), %r13
    mov (6*8)(%rsi), %r14
    mov (7*8)(%rsi), %r15

    ret
.size bootstrap_green_task,.-bootstrap_green_task

/* Mark that we don't need executable stack. */
.section .note.GNU-stack,"",%progbits
