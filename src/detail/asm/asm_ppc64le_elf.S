#define FRAMESIZE 32
#define REG_ARG_1 3
// unfortunately the IBM assembler just uses numbers for registers
// making the assembly hard to read when registers are mixed with offsets.
#define r0   0
#define r1   1
#define r2   2
#define r3   3
#define r4   4
#define r5   5
#define r6   6
#define r7   7
#define r8   8
#define r9   9
#define r10  10
#define r11  11
#define r12  12
#define r13  13
#define r14  14
#define r15  15
#define r16  16
#define r17  17
#define r18  18
#define r19  19
#define r20  20
#define r21  21
#define r22  22
#define r23  23
#define r24  24
#define r25  25
#define r26  26
#define r27  27
#define r28  28
#define r29  29
#define r30  30
#define r31  31

.text
.globl prefetch
.type prefetch,@function
.align 16
prefetch:
    // NOTE: dcbt prefetches data, not instructions!
    dcbt 0, REG_ARG_1
    blr
.size prefetch,.-prefetch

.text
.globl bootstrap_green_task
.type bootstrap_green_task,@function
.align 16
bootstrap_green_task:
    // mov rdi, r12     /* setup the function arg */
    // mov rsi, r13     /* setup the function arg */
    // and rsp, -16     /* align the stack pointer */
    // mov [rsp], r14   /* this is the new return adrress */
    mr r3, r14
    mr r4, r15
    mtlr r16
    blr
.size bootstrap_green_task,.-bootstrap_green_task

.text
.globl swap_registers
.type swap_registers,@function
.align 16
swap_registers:
    mflr r0
    std r0,-32(r3)
    mfcr r0
    std r0,8(r3)
    stdu r1,-FRAMESIZE(r1)
    // non-volatile registers: r1 (fp), r2 (toc), r13, r14-r31, f14-f31, v20-v31, vrsave,
    // arguments passed in r3-r10, stack
    // => previous reg list (r3), new reg list (r4)

    // saving non-volatile gprs:
    // std r2,   3*8(r3) // toc pointer
    // std r13,  4*8(r3) // thread pointer
    // std r14,  5*8(r3) // local vars
    // std r15,  6*8(r3)
    // std r16,  7*8(r3)
    // std r17,  8*8(r3)
    // std r18,  9*8(r3)
    // std r19, 10*8(r3)
    // std r20, 11*8(r3)
    // std r21, 12*8(r3)
    // std r22, 13*8(r3)
    // std r23, 14*8(r3)
    // std r24, 15*8(r3)
    // std r25, 16*8(r3)
    // std r26, 17*8(r3)
    // std r27, 18*8(r3)
    // std r28, 19*8(r3)
    // std r29, 20*8(r3)
    // std r30, 21*8(r3)
    // std r31, 22*8(r3) // end local vars
    
    // ld r0, 0(r4) // new link register
    // mtlr r0
    // ld r0, 8(r4)
    // mtcr r0

    // ld r2,   3*8(r4)
    // ld r13,  4*8(r4)
    // ld r14,  5*8(r4)
    // ld r15,  6*8(r4)
    // ld r16,  7*8(r4)
    // ld r17,  8*8(r4)
    // ld r18,  9*8(r4)
    // ld r19, 10*8(r4)
    // ld r20, 11*8(r4)
    // ld r21, 12*8(r4)
    // ld r22, 13*8(r4)
    // ld r23, 14*8(r4)
    // ld r24, 15*8(r4)
    // ld r25, 16*8(r4)
    // ld r26, 17*8(r4)
    // ld r27, 18*8(r4)
    // ld r28, 19*8(r4)
    // ld r29, 20*8(r4)
    // ld r30, 21*8(r4)
    // ld r31, 22*8(r4)
    
    blr

.size swap_registers,.-swap_registers

/* Mark that we don't need executable stack. */
.section .note.GNU-stack,"",%progbits