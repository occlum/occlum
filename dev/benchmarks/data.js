window.BENCHMARK_DATA = {
  "lastUpdate": 1671764718162,
  "repoUrl": "https://github.com/occlum/occlum",
  "entries": {
    "Sysbench Benchmark": [
      {
        "commit": {
          "author": {
            "email": "huaiqing.zq@antgroup.com",
            "name": "Zheng, Qi",
            "username": "qzheng527"
          },
          "committer": {
            "email": "volcano.dr@hotmail.com",
            "name": "volcano",
            "username": "volcano0dr"
          },
          "distinct": true,
          "id": "3723b5c23c8aab7d96285c5f5646bf3f07213d28",
          "message": "[ci] enable push tigger for benchmark ci",
          "timestamp": "2022-12-22T10:37:38+08:00",
          "tree_id": "89d17ca6a4a547c570f5f7e3647de9a35a0905a9",
          "url": "https://github.com/occlum/occlum/commit/3723b5c23c8aab7d96285c5f5646bf3f07213d28"
        },
        "date": 1671678721461,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "Threads Minimum latency",
            "value": 0.09,
            "unit": "ms",
            "extra": "min"
          },
          {
            "name": "Threads Average Latency",
            "value": 327.53,
            "unit": "ms",
            "extra": "avg"
          },
          {
            "name": "Threads Maximum Latency",
            "value": 61712.05,
            "unit": "ms",
            "extra": "max"
          },
          {
            "name": "Thread 95th Percentile Latency",
            "value": 320.17,
            "unit": "ms",
            "extra": "per95"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "huaiqing.zq@antgroup.com",
            "name": "Zheng, Qi",
            "username": "qzheng527"
          },
          "committer": {
            "email": "volcano.dr@hotmail.com",
            "name": "volcano",
            "username": "volcano0dr"
          },
          "distinct": true,
          "id": "01f161840cf71eeea0e49b342bdcfa4fa7323625",
          "message": "[ci] Update benchmark links and increase timeout time",
          "timestamp": "2022-12-23T09:24:59+08:00",
          "tree_id": "7a7d1382489d11d2a910aa1b928066002486dac8",
          "url": "https://github.com/occlum/occlum/commit/01f161840cf71eeea0e49b342bdcfa4fa7323625"
        },
        "date": 1671764717164,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "Threads Minimum latency",
            "value": 0.07,
            "unit": "ms",
            "extra": "min"
          },
          {
            "name": "Threads Average Latency",
            "value": 49.59,
            "unit": "ms",
            "extra": "avg"
          },
          {
            "name": "Threads Maximum Latency",
            "value": 567.75,
            "unit": "ms",
            "extra": "max"
          },
          {
            "name": "Thread 95th Percentile Latency",
            "value": 253.35,
            "unit": "ms",
            "extra": "per95"
          }
        ]
      }
    ],
    "FIO Benchmark": [
      {
        "commit": {
          "author": {
            "email": "huaiqing.zq@antgroup.com",
            "name": "Zheng, Qi",
            "username": "qzheng527"
          },
          "committer": {
            "email": "volcano.dr@hotmail.com",
            "name": "volcano",
            "username": "volcano0dr"
          },
          "distinct": true,
          "id": "01f161840cf71eeea0e49b342bdcfa4fa7323625",
          "message": "[ci] Update benchmark links and increase timeout time",
          "timestamp": "2022-12-23T09:24:59+08:00",
          "tree_id": "7a7d1382489d11d2a910aa1b928066002486dac8",
          "url": "https://github.com/occlum/occlum/commit/01f161840cf71eeea0e49b342bdcfa4fa7323625"
        },
        "date": 1671763960821,
        "tool": "customBiggerIsBetter",
        "benches": [
          {
            "name": "Sequential Write Throughput",
            "value": 47.8,
            "unit": "MiB/s",
            "extra": "seqwrite"
          },
          {
            "name": "Random Write Throughput",
            "value": 42.3,
            "unit": "MiB/s",
            "extra": "randwrite"
          },
          {
            "name": "Sequential Read Throughput",
            "value": 284,
            "unit": "MiB/s",
            "extra": "seqread"
          },
          {
            "name": "Random Read Throughput",
            "value": 185,
            "unit": "MiB/s",
            "extra": "randread"
          }
        ]
      }
    ],
    "Iperf3 Benchmark": [
      {
        "commit": {
          "author": {
            "email": "huaiqing.zq@antgroup.com",
            "name": "Zheng, Qi",
            "username": "qzheng527"
          },
          "committer": {
            "email": "volcano.dr@hotmail.com",
            "name": "volcano",
            "username": "volcano0dr"
          },
          "distinct": true,
          "id": "01f161840cf71eeea0e49b342bdcfa4fa7323625",
          "message": "[ci] Update benchmark links and increase timeout time",
          "timestamp": "2022-12-23T09:24:59+08:00",
          "tree_id": "7a7d1382489d11d2a910aa1b928066002486dac8",
          "url": "https://github.com/occlum/occlum/commit/01f161840cf71eeea0e49b342bdcfa4fa7323625"
        },
        "date": 1671764343354,
        "tool": "customBiggerIsBetter",
        "benches": [
          {
            "name": "Sender Average Rate",
            "value": 3658,
            "unit": "Mbits/sec",
            "extra": "sender"
          },
          {
            "name": "Receiver Average Rate",
            "value": 3657,
            "unit": "Mbits/sec",
            "extra": "receiver"
          }
        ]
      }
    ]
  }
}