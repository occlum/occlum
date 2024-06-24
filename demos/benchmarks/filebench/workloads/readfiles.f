# A simple readfiles workload

define fileset name="testF",entries=16,filesize=4k,path="/ext2/fbtest_ext2",prealloc
define process name="readerP",instances=1 {
thread name="readerT",instances=1 {
    flowop openfile name="openOP",filesetname="testF"
    flowop readwholefile name="readOP",filesetname="testF"
    flowop closefile name="closeOP"
}
}
run 30
