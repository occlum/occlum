# Python multiprocessing package provides API for process communication and management.
# This demo demonstrates creating a worker pool and offloading jobs.
# Processes communicate through shared memory (POSIX).
import multiprocessing as mp
import time

def job():
    print(1)

start = time.time()

if __name__ == '__main__':
    mp.set_start_method('spawn')
    pool = mp.Pool(processes=4)
    for i in range(4):
        pool.apply(job)
        print("total time {}".format(time.time() - start))

