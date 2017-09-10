.code

prefetch_asm PROC FRAME
    .endprolog
    prefetcht1 [rcx]
    ret

prefetch_asm ENDP
END

.code

bootstrap_green_task PROC FRAME
    .endprolog
    mov r12, rcx     /* setup the function arg */
    mov r13, rdx     /* setup the function arg */
    mov r14, [rsp+8] /* this is the new return adrress */
    ret

bootstrap_green_task ENDP
END

.code

swap_registers PROC FRAME
    .endprolog
    mov rbx, [rcx + 0*8]
    mov rsp, [rcx + 1*8]
    mov rbp, [rcx + 2*8]
    mov r12, [rcx + 4*8]
    mov r13, [rcx + 5*8]
    mov r14, [rcx + 6*8]
    mov r15, [rcx + 7*8]
    mov rdi, [rcx + 9*8]
    mov rsi, [rcx + 10*8]

    /* Save non-volatile XMM registers: */
    movapd xmm6, [rcx + 16*8]
    movapd xmm7, [rcx + 18*8]

    /* load NT_TIB */
    movq gs:(0x30), r10
    /* save current stack base */
    movq [r10 + 0x08], rax
    mov  rax, [rcx + 11*8]
    /* save current stack limit */
    movq  [r10 + 0x10], rax
    mov  rax, [rcx + 12*8]
    /* save current deallocation stack */
    movq  [r10 + 0x1478], %rax
    mov  rax, [rcx + 13*8]
    /* save fiber local storage */
    // movq  [r10 + 0x18], rax
    // mov  rax, [rcx + 14*8]

    mov rcx, [rcx + 3*8] 

    mov [rdx + 0*8], rbx
    mov [rdx + 1*8], rsp
    mov [rdx + 2*8], rbp
    mov [rdx + 4*8], r12
    mov [rdx + 5*8], r13
    mov [rdx + 6*8], r14
    mov [rdx + 7*8], r15
    mov [rdx + 9*8], rdi
    mov [rdx + 10*8], rsi

    // Restore non-volatile XMM registers:
    movapd [rdx + 16*8], xmm6
    movapd [rdx + 18*8], xmm7

    /* load NT_TIB */
    movq  gs:(0x30), r10
    /* restore fiber local storage */
    // mov [rdx + 14*8], rax
    // movq rax, [r10 + 0x18]
    /* restore deallocation stack */
    mov [rdx + 13*8], rax
    movq rax, [r10 + 0x1478]
    /* restore stack limit */
    mov [rdx + 12*8], rax
    movq rax, [r10 + 0x10]
    /* restore stack base */
    mov [rdx + 11*8], rax
    movq rax, [r10 + 0x08]

    mov [rdx + 3*8], rcx
    ret

swap_registers ENDP
END




