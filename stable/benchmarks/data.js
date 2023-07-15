window.BENCHMARK_DATA = {
  "lastUpdate": 1689459236478,
  "repoUrl": "https://github.com/occlum/occlum",
  "entries": {
    "Iperf3 Benchmark": [
      {
        "commit": {
          "author": {
            "name": "wang384670111",
            "username": "wang384670111"
          },
          "committer": {
            "name": "wang384670111",
            "username": "wang384670111"
          },
          "id": "2772d9f7dbf7d12893b299bed1f9c4acc4bd7179",
          "message": "Adjust the Ordering for all the atomics",
          "timestamp": "2023-06-28T06:21:26Z",
          "url": "https://github.com/occlum/occlum/pull/1334/commits/2772d9f7dbf7d12893b299bed1f9c4acc4bd7179"
        },
        "date": 1689217874888,
        "tool": "customBiggerIsBetter",
        "benches": [
          {
            "name": "Sender Average Rate",
            "value": 819,
            "unit": "Mbits/sec",
            "extra": "sender"
          },
          {
            "name": "Receiver Average Rate",
            "value": 819,
            "unit": "Mbits/sec",
            "extra": "receiver"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Qi Zheng",
            "username": "qzheng527",
            "email": "huaiqing.zq@antgroup.com"
          },
          "committer": {
            "name": "volcano",
            "username": "volcano0dr",
            "email": "volcano.dr@hotmail.com"
          },
          "id": "762897f9ea36bf47e36207bd012378396e177fc2",
          "message": "[docs] Update description for benchmark ci and pages",
          "timestamp": "2023-07-13T09:30:52Z",
          "url": "https://github.com/occlum/occlum/commit/762897f9ea36bf47e36207bd012378396e177fc2"
        },
        "date": 1689459235391,
        "tool": "customBiggerIsBetter",
        "benches": [
          {
            "name": "Sender Average Rate",
            "value": 2005,
            "unit": "Mbits/sec",
            "extra": "sender"
          },
          {
            "name": "Receiver Average Rate",
            "value": 2005,
            "unit": "Mbits/sec",
            "extra": "receiver"
          }
        ]
      }
    ],
    "Sysbench Benchmark": [
      {
        "commit": {
          "author": {
            "name": "wang384670111",
            "username": "wang384670111"
          },
          "committer": {
            "name": "wang384670111",
            "username": "wang384670111"
          },
          "id": "2772d9f7dbf7d12893b299bed1f9c4acc4bd7179",
          "message": "Adjust the Ordering for all the atomics",
          "timestamp": "2023-06-28T06:21:26Z",
          "url": "https://github.com/occlum/occlum/pull/1334/commits/2772d9f7dbf7d12893b299bed1f9c4acc4bd7179"
        },
        "date": 1689217884663,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "Threads Minimum latency",
            "value": 0.56,
            "unit": "ms",
            "extra": "min"
          },
          {
            "name": "Threads Average Latency",
            "value": 362.7,
            "unit": "ms",
            "extra": "avg"
          },
          {
            "name": "Threads Maximum Latency",
            "value": 5953.21,
            "unit": "ms",
            "extra": "max"
          },
          {
            "name": "Thread 95th Percentile Latency",
            "value": 1479.41,
            "unit": "ms",
            "extra": "per95"
          }
        ]
      }
    ]
  }
}