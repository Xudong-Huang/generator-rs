// s390x (z/Architecture) ELF ABI context switch.
//
// Call-saved state as defined by the ELF ABI (and what glibc's setjmp saves):
//   general purpose registers r6-r13, the return address r14 and the stack
//   pointer r15, plus the floating point registers f8-f15.
// The access registers hold the thread pointer, which is shared by all
// contexts running on a thread, so they are left alone.
//
// The `Registers` layout matches the order STMG/LMG use, so the whole gpr
// range can be transferred with a single instruction:
//   0: r6, 1: r7, 2: r8, 3: r9, 4: r10, 5: r11, 6: r12, 7: r13,
//   8: r14, 9: r15, 10-17: f8-f15

.text
.globl prefetch
.type prefetch,@function
.balign 16
prefetch:
    pfd 1, 0(%r2)  // prefetch for load
    br %r14
.size prefetch,.-prefetch

.text
.globl bootstrap_green_task
.type bootstrap_green_task,@function
.balign 16
bootstrap_green_task:
    lgr %r2, %r7   // arg0
    lgr %r3, %r8   // arg1
    lghi %r14, 0   // clear the return address
    br %r9         // enter the generator
.size bootstrap_green_task,.-bootstrap_green_task

.text
.globl swap_registers
.type swap_registers,@function
.balign 16
swap_registers:
    // r2 = out_regs, r3 = in_regs

    stmg %r6, %r15, 0(%r2)
    std %f8, 10*8(%r2)
    std %f9, 11*8(%r2)
    std %f10, 12*8(%r2)
    std %f11, 13*8(%r2)
    std %f12, 14*8(%r2)
    std %f13, 15*8(%r2)
    std %f14, 16*8(%r2)
    std %f15, 17*8(%r2)

    // r3 is outside of the r6-r15 range restored below, so it stays a valid
    // base register for the whole restore sequence
    ld %f8, 10*8(%r3)
    ld %f9, 11*8(%r3)
    ld %f10, 12*8(%r3)
    ld %f11, 13*8(%r3)
    ld %f12, 14*8(%r3)
    ld %f13, 15*8(%r3)
    ld %f14, 16*8(%r3)
    ld %f15, 17*8(%r3)
    lmg %r6, %r15, 0(%r3)

    br %r14
.size swap_registers,.-swap_registers

/* Mark that we don't need executable stack. */
.section .note.GNU-stack,"",%progbits
