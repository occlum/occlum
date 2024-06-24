#
# CDDL HEADER START
#
# The contents of this file are subject to the terms of the
# Common Development and Distribution License (the "License").
# You may not use this file except in compliance with the License.
#
# You can obtain a copy of the license at usr/src/OPENSOLARIS.LICENSE
# or http://www.opensolaris.org/os/licensing.
# See the License for the specific language governing permissions
# and limitations under the License.
#
# When distributing Covered Code, include this CDDL HEADER in each
# file and include the License file at usr/src/OPENSOLARIS.LICENSE.
# If applicable, add the following below this CDDL HEADER, with the
# fields enclosed by brackets "[]" replaced with your own identifying
# information: Portions Copyright [yyyy] [name of copyright owner]
#
# CDDL HEADER END
#
#
# Copyright 2009 Sun Microsystems, Inc.  All rights reserved.
# Use is subject to license terms.
#

# Workload "oltp" from filebench cannot run directly on Occlum
# since aio write/wait and semaphore-related flowops are not supported.
# We replace some flowops to achieve similar performance quota.

set $dir=/root/fbtest
set $eventrate=0
set $iosize=2k
set $nshadows=200
set $ndbwriters=10
set $usermode=200000
set $filesize=10m
set $memperthread=1m
set $workingset=0
set $logfilesize=10m
set $nfiles=10
set $nlogfiles=1
set $directio=0
eventgen rate = $eventrate

# Define a datafile and logfile
define fileset name=datafiles,path=$dir,size=$filesize,entries=$nfiles,dirwidth=1024,prealloc=100,reuse
define fileset name=logfile,path=$dir,size=$logfilesize,entries=$nlogfiles,dirwidth=1024,prealloc=100,reuse

define process name=lgwr,instances=1
{
  thread name=lgwr,memsize=$memperthread
  {
    flowop write name=lg-write,filesetname=logfile,
        iosize=256k,random,directio=$directio,dsync
  }
}

# Define database writer processes
define process name=dbwr,instances=$ndbwriters
{
  thread name=dbwr,memsize=$memperthread
  {
    flowop write name=dbwrite-a,filesetname=datafiles,
        iosize=$iosize,workingset=$workingset,random,iters=100,opennext,directio=$directio,dsync
    flowop hog name=dbwr-hog,value=10000
  }
}

define process name=shadow,instances=$nshadows
{
  thread name=shadow,memsize=$memperthread
  {
    flowop read name=shadowread,filesetname=datafiles,
        iosize=$iosize,workingset=$workingset,random,opennext,directio=$directio
    flowop hog name=shadowhog,value=$usermode
    flowop eventlimit name=random-rate
  }
}

echo "OLTP Version 3.0  personality successfully loaded"

run 60
