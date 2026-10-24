# t4: ORG/EQU, shifts, negative semantics
base:   EQU 16
        ORG base
start:  addi x1, x0, -1
        srai x2, x1, 4
        srli x3, x1, 4
        addi x4, x0, -7
        srai x5, x4, 1
        div  x6, x4, x1
        rem  x7, x4, x1
        divu x8, x1, x4
        mul  x9, x4, x1
        mulh x10, x4, x1
        mulhsu x11, x4, x1
        mulhu x12, x4, x1
        sltiu x13, x1, -1
        sltu x14, x0, x1
        slti x15, x4, -6
        andi x16, x1, 0xf
        ori x17, x0, 0x7f0
        xori x18, x1, 0x0f
        lui x19, 0x12345
        auipc x20, 0x1
        jal x21, target
        ebreak x0, x0, 0
target: jalr x22, x21, 0
        sub x23, x1, x4
        addw x24, x1, x1
        subw x25, x0, x1
        sllw x26, x1, x4
        sraw x27, x1, x4
        ebreak x0, x0, 0
