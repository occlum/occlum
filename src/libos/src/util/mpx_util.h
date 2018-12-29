#ifndef __MPX_UTIL__
#define __MPX_UTIL__

#ifdef __cplusplus
extern "C" {
#endif

/* Enable the use of MPX bound registers bnd0-bnd3 and bound instructions
 * bndmk, bndcl and bndcu. */
int __mpx_enable(void);

/* Make a new bound in bnd<i>*/
void __mpx_bndmk0(unsigned long base, unsigned long size);
void __mpx_bndmk1(unsigned long base, unsigned long size);
void __mpx_bndmk2(unsigned long base, unsigned long size);
void __mpx_bndmk3(unsigned long base, unsigned long size);

/* Check x against the lower bound of bnd<i>*/
void __mpx_bndcl0(unsigned long x);
void __mpx_bndcl1(unsigned long x);
void __mpx_bndcl2(unsigned long x);
void __mpx_bndcl3(unsigned long x);

/* Check x against the upper bound of bnd<i> */
void __mpx_bndcu0(unsigned long x);
void __mpx_bndcu1(unsigned long x);
void __mpx_bndcu2(unsigned long x);
void __mpx_bndcu3(unsigned long x);

#ifdef __cplusplus
}
#endif

#endif /* __MPX_UTIL__ */
