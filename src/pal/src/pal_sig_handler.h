#ifndef __PAL_SIG_HANDLER_H__
#define __PAL_SIG_HANDLER_H__

// Register signal handlers for PAL.
//
// Currently, there is only one signal number that needs to be covered: signal
// 64, which is used to notify interrupts (see LibOS code for more info). For
// a hardware-mode enclave, the signal is handled by the signal handlers
// registered by Intel SGX SDK. So we are ok in this case. But for a
// simulation-mode enclave, there is no signal handler registered by Intel SGX
// SDK. Without a signal handler, the delivery of the signal will kill the
// process. This crash can be prevented by this API.
int pal_register_sig_handlers(void);

#endif /* __PAL_SIG_HANDLER_H__ */
