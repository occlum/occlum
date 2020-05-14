package main

import "net/http"
import "log"
import "flag"
import "fmt"

type Controller struct {}
func (c Controller)ServeHTTP(writer http.ResponseWriter, request *http.Request){
    writer.Write([]byte("hello,1\n"));
}

func hello(writer http.ResponseWriter, request *http.Request) {
    writer.Write([]byte("hello,2\n"));
}

var port string

func init() {
        flag.StringVar(&port, "port", "8090", "port number, default value is 8090")
}

func main(){
    flag.Parse()
    fmt.Println("Web Server port is:", port)
    http.Handle("/hello1",&Controller{})
    http.Handle("/hello2",http.HandlerFunc(hello))
    log.Fatal(http.ListenAndServe(":" + port, nil))
}
