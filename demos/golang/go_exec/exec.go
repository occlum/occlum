package main

import (
	"bytes"
	"fmt"
	"os/exec"

	_ "github.com/gin-gonic/gin"
)

func main() {
	var stdout, stderr bytes.Buffer
	cmd := exec.Command("/root/helloworld", "a")
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	if err != nil {
		fmt.Printf("cmd.Run() failed with %s\n", stderr.String())
	} else {
		fmt.Printf("cmd.Run() succeed with %s\n", stdout.String())
	}
}
