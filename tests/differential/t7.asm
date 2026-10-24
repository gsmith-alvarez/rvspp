# t7: runtime error paths
        addi x1, x0, 7
        div  x2, x1, x0
        ebreak x0, x0, 0
