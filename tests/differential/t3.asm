# t3: inputs, loop, mul/div (directives kept up front)
p1:     DC "n= "
p2:     DC "f= "
p3:     DC "h= "
        ld x1, p1(x0)
        ecall x1, x1, 5
        ecall x0, x1, 0
        ecall x0, x1, 2
        ld x2, p2(x0)
        ecall x2, x2, 6
        ecall x0, x2, 1
        ld x3, p3(x0)
        ecall x3, x3, 7
        ecall x0, x3, 2
        # sum 1..n loop
        addi x4, x0, 0
        addi x5, x0, 1
loop:   bge x5, x1, done
        add x4, x4, x5
        addi x5, x5, 1
        jal x0, loop
done:   ecall x0, x4, 0
        # mul / div signed and unsigned
        addi x6, x0, -7
        addi x7, x0, 3
        mul  x8, x6, x7
        ecall x0, x8, 0
        div  x9, x6, x7
        ecall x0, x9, 0
        rem  x10, x6, x7
        ecall x0, x10, 0
        mulh x11, x6, x7
        ecall x0, x11, 2
        divu x12, x6, x7
        ecall x0, x12, 0
        div  x14, x6, x1
        rem  x15, x6, x1
        ecall x0, x14, 0
        ecall x0, x15, 0
        sltiu x16, x1, -1
        ecall x0, x16, 0
        sltu x17, x0, x1
        ecall x0, x17, 0
        ebreak x0, x0, 0
