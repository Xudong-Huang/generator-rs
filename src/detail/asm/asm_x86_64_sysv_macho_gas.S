.text
.globl _prefetch
.align 8
_prefetch:
    prefetcht2 (%rdi)
    ret

.text
.globl _bootstrap_green_task
.align 8
_bootstrap_green_task:
    mov %r12, %rdi     /* setup the function arg */
    mov %r13, %rsi     /* setup the function arg */
    and $-16, %rsp     /* align the stack pointer */
    mov %r14, (%rsp)   /* this is the new return adrress */
    ret

.text
.globl _swap_registers
.align 8
_swap_registers:
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
