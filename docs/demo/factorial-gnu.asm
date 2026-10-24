.data
msg:    .asciz "fact(5) = "
.text
.globl main
main:   li    sp, 0x1000
        li    a0, 5
        jal   ra, fact
        la    t1, msg
        mv    a1, t1
        ecall a1, a1, 4
        ecall x0, a0, 0
        ebreak x0, x0, 0

fact:   addi  sp, sp, -16
        sd    ra, 8(sp)
        sd    s0, 0(sp)
        mv    s0, a0
        li    t0, 1
        bge   t0, s0, base
        addi  a0, s0, -1
        jal   ra, fact
        mul   a0, a0, s0
        j     done
base:   li    a0, 1
done:   ld    ra, 8(sp)
        ld    s0, 0(sp)
        addi  sp, sp, 16
        ret
