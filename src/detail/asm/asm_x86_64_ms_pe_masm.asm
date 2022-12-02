.code

prefetch_asm PROC FRAME
    .endprolog
    prefetcht2 [rcx]
    ret
prefetch_asm ENDP


bootstrap_green_task PROC FRAME
    .endprolog
    mov rcx, r12     ; setup the function arg
    mov rdx, r13     ; setup the function arg
    and rsp, -16     ; align the stack pointer
    mov [rsp], r14   ; this is the new return adrress
    ret
bootstrap_green_task ENDP


swap_registers PROC FRAME
    .endprolog
    mov [rcx + 0*8], rbx
    mov [rcx + 1*8], rsp
    mov [rcx + 2*8], rbp
    mov [rcx + 4*8], r12
    mov [rcx + 5*8], r13
    mov [rcx + 6*8], r14
    mov [rcx + 7*8], r15
    mov [rcx + 9*8], rdi
    mov [rcx + 10*8], rsi

    mov r10, rcx
    and r10, not 8

    ; Save non-volatile XMM registers:
    movapd [r10 + 16*8], xmm6
    movapd [r10 + 18*8], xmm7
    movapd [r10 + 20*8], xmm8
    movapd [r10 + 22*8], xmm9
    movapd [r10 + 24*8], xmm10
    movapd [r10 + 26*8], xmm11
    movapd [r10 + 28*8], xmm12
    movapd [r10 + 30*8], xmm13
    movapd [r10 + 32*8], xmm14
    movapd [r10 + 34*8], xmm15

    ; load NT_TIB
    mov r10, gs:[030h]
    ; save current stack base
    mov rax, [r10 + 08h]
    mov [rcx + 11*8],  rax
    ; save current stack limit
    mov rax,  [r10 + 010h]
    mov [rcx + 12*8], rax
    ; save current deallocation stack
    mov rax, [r10 + 01478h]
    mov [rcx + 13*8], rax
    ; save fiber local storage
    ; mov rax, [r10 + 0x18]
    ; mov [rcx + 14*8], rax

    ; mov [rcx + 3*8], rcx

    mov rbx, [rdx + 0*8]
    mov rsp, [rdx + 1*8]
    mov rbp, [rdx + 2*8]
    mov r12, [rdx + 4*8]
    mov r13, [rdx + 5*8]
    mov r14, [rdx + 6*8]
    mov r15, [rdx + 7*8]
    mov rdi, [rdx + 9*8]
    mov rsi, [rdx + 10*8]

    mov r10, rdx
    and r10, not 8

    ; Restore non-volatile XMM registers:
    movapd xmm6, [r10 + 16*8]
    movapd xmm7, [r10 + 18*8]
    movapd xmm8, [r10 + 20*8]
    movapd xmm9, [r10 + 22*8]
    movapd xmm10, [r10 + 24*8]
    movapd xmm11, [r10 + 26*8]
    movapd xmm12, [r10 + 28*8]
    movapd xmm13, [r10 + 30*8]
    movapd xmm14, [r10 + 32*8]
    movapd xmm15, [r10 + 34*8]

    ; load NT_TIB
    mov r10, gs:[030h]
    ; restore fiber local storage
    ; mov [rdx + 14*8], rax
    ; movq rax, [r10 + 0x18]
    ; restore deallocation stack
    mov rax, [rdx + 13*8]
    mov [r10 + 01478h], rax
    ; restore stack limit
    mov rax, [rdx + 12*8]
    mov [r10 + 010h], rax
    ; restore stack base
    mov rax, [rdx + 11*8]
    mov [r10 + 08h], rax

    ; mov rcx, [rdx + 3*8]
    ret
swap_registers ENDP

END

