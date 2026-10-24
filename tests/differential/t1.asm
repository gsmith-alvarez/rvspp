# t1: arithmetic, memory, branch, print
        addi x1, x0, 5
        addi x2, x0, 7
        add  x3, x1, x2
        ecall x0, x3, 0
        sub  x4, x2, x1
        ecall x0, x4, 0
        slli x5, x1, 2
        ecall x0, x5, 0
        sd   x3, 0(x0)
        ld   x6, 0(x0)
        ecall x0, x6, 0
        beq  x1, x2, 16
        ecall x0, x0, 0
        ebreak x0, x0, 0
        ecall x0, x1, 0
