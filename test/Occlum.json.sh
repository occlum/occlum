#!/bin/bash
bin_sefs_mac=$1
lib_sefs_mac=$2

cat <<EOF
{
    "vm": {
        "user_space_size": "128MB"
    },
    "process": {
        "default_stack_size": "4MB",
        "default_heap_size": "16MB",
        "default_mmap_size": "32MB"
    },
    "mount": [
        {
            "target": "/",
            "type": "sefs",
            "source": "./sefs/root"
        },
        {
            "target": "/bin",
            "type": "sefs",
            "source": "./sefs/bin",
            "options": {
                "integrity_only": true,
                "MAC": "$bin_sefs_mac"
            }
        },
        {
            "target": "/lib",
            "type": "sefs",
            "source": "./sefs/lib",
            "options": {
                "integrity_only": true,
                "MAC": "$lib_sefs_mac"
            }
        },
        {
            "target": "/host",
            "type": "hostfs",
            "source": "."
        },
        {
            "target": "/tmp",
            "type": "ramfs"
        }
    ]
}
EOF
