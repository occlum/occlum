# A simple readfiles workload

define fileset name="testF",entries=10,filesize=1k,path="/tmp",prealloc
define process name="readerP",instances=1 {
thread name="readerT",instances=1 {
    flowop openfile name="openOP",filesetname="testF"
    flowop readwholefile name="readOP",filesetname="testF"
    flowop closefile name="closeOP"
}
}
run 30
