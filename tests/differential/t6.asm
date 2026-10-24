# t6: rewrite improvements (not expected to match RVS046 exactly)
        addi x1, x0, 5
        remu x2, x1, x1
        ecall x0, x2, 0
        addiw x3, x1, -1
        ecall x0, x3, 0
        addw x4, x1, x1
        mulw x5, x4, x4
        ecall x0, x5, 0
        divw x6, x4, x1
        ecall x0, x6, 0
        remw x7, x1, x4
        ecall x0, x7, 0
        divuw x8, x4, x1
        remuw x9, x4, x1
        ecall x0, x9, 0
        subi x10, x1, 1
        ecall x0, x10, 0
        ecall
        ebreak
