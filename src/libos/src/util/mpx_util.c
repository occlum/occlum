#include "mpx_util.h"
#include <string.h>
#include <stdint.h>

/*
 * Define data structures for the part of XSAVE area that are relevant
 * to enabling MPX
 */

typedef struct {
    uint64_t xstate_bv;
    uint64_t __irrelevant[2];
    uint64_t __reserved[5];
} __attribute__ ((packed)) xsave_header_t;

typedef struct {
    uint64_t __irrelevant[8];
} __attribute__ ((packed)) bndreg_t ;

typedef struct {
    uint64_t enable: 1;
    uint64_t bndpreserve: 1;
    uint64_t __reserved: 10;
    uint64_t __irrelevant: 52;
} __attribute__ ((packed)) bndcfgu_t;

typedef struct {
    bndcfgu_t bndcfgu;
    uint64_t __irrelevant;
} __attribute__((packed)) bndcsr_t;

typedef struct {
    uint8_t __irrelevant0[512];
    xsave_header_t header;
    uint8_t __irrelevant1[256];
    uint8_t __irrelevant2[128];
    bndreg_t bndreg;
    bndcsr_t bndcsr;
} __attribute__ ((packed, __aligned__(64))) xsave_area_t ;


/*
 * Restore the state components of CPU specified in rfbm from xsave_area.
 *
 * rfbm is the requested-feature bitmap, whose bits specifies which state
 * components are to restored by this instruction.
 */
static void xrstor(xsave_area_t *xsave_area, uint64_t rfbm) {
#define REX_PREFIX  "0x48, "
#define XRSTOR64    REX_PREFIX "0x0f,0xae,0x2f "

    __asm__ __volatile__ (
        ".byte " XRSTOR64 "\n\t"
        :
        : "D" (xsave_area), "m" (*xsave_area), "a" (rfbm), "d" (rfbm)
        : "memory");
}

/* The state component bitmaps for MPX
 *
 * The state component 3 (i.e., bit 3) is BNDREG state, which consists of the
 * four MPX bound registers BND0-BND3.
 *
 * The state component 4 (i.e., bit 4) is BNDCSR state, which consists of the
 * one MPX configuration register BNDCFGU and one MPX status register
 * BNDSTATUS.
 * */
#define MPX_BNDREG_COMPONENT_MASK       (0x08UL)
#define MPX_BNDCSR_COMPONENT_MASK       (0x10UL)
#define MPX_ALL_COMPONENT_MASK          (MPX_BNDCSR_COMPONENT_MASK | \
                                         MPX_BNDREG_COMPONENT_MASK)

int __mpx_enable(void) {
    xsave_area_t xsave_area;
    memset(&xsave_area, 0, sizeof(xsave_area));

    /* Initialize MPX states
     *
     * xrestor initializes state component i if rfbm[i] = 1 and
     * xsave_area.header.xstate_bv[i] = 0 */
    uint64_t rfbm = MPX_ALL_COMPONENT_MASK;
    xrstor(&xsave_area, rfbm);

    /* xrestor updates state component i if rfbm[i] = 1 and
     * xsave_area.header.xstate_bv[i] = 1 */
    xsave_area.header.xstate_bv = MPX_BNDCSR_COMPONENT_MASK;
    /* Set enable bit to 1 to enable MPX */
    xsave_area.bndcsr.bndcfgu.enable = 1;
    /* Set bndpreserve bit to 1 so that BND0-BND3 remain unchanged upon
     * control flow transfer instructions (e.g., call, jmp, etc.). */
    xsave_area.bndcsr.bndcfgu.bndpreserve = 1;
    /* Set BNDCSR state component so that MPX is enabled. */
    rfbm = MPX_BNDCSR_COMPONENT_MASK;
    xrstor(&xsave_area, rfbm);

    return 0;
}
