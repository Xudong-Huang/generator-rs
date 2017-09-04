.code

prefetch PROC FRAME
    .endprolog
    prefetcht1 [%rcx]

prefetch ENDP
END


