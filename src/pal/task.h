#ifndef __TASK_H_
#define __TASK_H_

int run_new_task(sgx_enclave_id_t eid);
int wait_all_tasks(void);
int wait_main_task(void);

#endif /* __TASK_H_ */
