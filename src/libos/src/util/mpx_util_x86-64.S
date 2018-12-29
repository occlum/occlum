    .file "mpx_util_x86-64.S"

/* For bnd0 */

    .global __mpx_bndmk0
    .type __mpx_bndmk0, @function
__mpx_bndmk0:
    bndmk (%rdi, %rsi, 1), %bnd0
    ret

    .global __mpx_bndcl0
    .type __mpx_bndcl0, @function
__mpx_bndcl0:
    bndcl %rdi, %bnd0
    ret

    .global __mpx_bndcu0
    .type __mpx_bndcu0, @function
__mpx_bndcu0:
    bndcu %rdi, %bnd0
    ret

/* For bnd1 */

    .global __mpx_bndmk1
    .type __mpx_bndmk1, @function
__mpx_bndmk1:
    bndmk (%rdi, %rsi, 1), %bnd1
    ret

    .global __mpx_bndcl1
    .type __mpx_bndcl1, @function
__mpx_bndcl1:
    bndcl %rdi, %bnd1
    ret

    .global __mpx_bndcu1
    .type __mpx_bndcu1, @function
__mpx_bndcu1:
    bndcu %rdi, %bnd1
    ret

/* For bnd2 */

    .global __mpx_bndmk2
    .type __mpx_bndmk2, @function
__mpx_bndmk2:
    bndmk (%rdi, %rsi, 1), %bnd2
    ret

    .global __mpx_bndcl2
    .type __mpx_bndcl2, @function
__mpx_bndcl2:
    bndcl %rdi, %bnd2
    ret

    .global __mpx_bndcu2
    .type __mpx_bndcu2, @function
__mpx_bndcu2:
    bndcu %rdi, %bnd2
    ret

/* For bnd3 */

    .global __mpx_bndmk3
    .type __mpx_bndmk3, @function
__mpx_bndmk3:
    bndmk (%rdi, %rsi, 1), %bnd3
    ret

    .global __mpx_bndcl3
    .type __mpx_bndcl3, @function
__mpx_bndcl3:
    bndcl %rdi, %bnd3
    ret

    .global __mpx_bndcu3
    .type __mpx_bndcu3, @function
__mpx_bndcu3:
    bndcu %rdi, %bnd3
    ret
