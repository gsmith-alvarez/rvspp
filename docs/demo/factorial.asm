# factorial(5) - recursive, with real stack frames
msg:    DC    "fact(5) = "
start:  lui   x2, 1              # sp = 0x1000
        addi  x10, x0, 5        # n = 5
        jal   ra, fact
        addi  x5, x0, msg
        ecall x5, x5, 4         # print the message
        ecall x0, x10, 0        # print the result
        ebreak x0, x0, 0

fact:   addi  x2, x2, -16       # allocate a stack frame
        sd    ra, 8(x2)         # save ra
        sd    s0, 0(x2)         # save s0
        add   s0, x10, x0
        addi  t0, x0, 1
        bge   t0, s0, base
        addi  x10, s0, -1
        jal   ra, fact
        mul   x10, x10, s0
        jal   x0, done
base:   addi  x10, x0, 1
done:   ld    ra, 8(x2)
        ld    s0, 0(x2)
        addi  x2, x2, 16        # release the frame
        jalr  x0, ra, 0
