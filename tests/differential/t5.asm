# t5: ecall 9 string input
        addi x9, x0, 64
        ecall x9, x0, 9
        ecall x0, x9, 4
        ebreak x0, x0, 0
        ORG 64
buf:    DM 2
