# t2: directives, strings, floats
msg:    DC "hello world"
vals:   DW 1, 2, -3
half:   DH 10, 20
bytes:  DB 1, 2, 255
flts:   DF 1.5, -2.25
store:  DM 3
        ld x1, msg(x0)
        ecall x0, x1, 4
        ld x2, vals(x0)
        ecall x0, x2, 2
        ld x3, vals+8(x0)
        ecall x0, x3, 0
        lh x4, half+2(x0)
        ecall x0, x4, 0
        lb x5, bytes+2(x0)
        ecall x0, x5, 0
        fld f1, flts(x0)
        ecall x0, f1, 1
        fld f2, flts+8(x0)
        fadd.d f3, f1, f2
        ecall x0, f3, 1
        fsqrt.d f4, f1
        ecall x0, f4, 1
        fmul.d f5, f1, f2
        ecall x0, f5, 1
        ebreak x0, x0, 0
