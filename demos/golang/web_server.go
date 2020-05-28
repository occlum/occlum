package main

import (
    "io"
    "os"

    "github.com/gin-gonic/gin"
)

func main() {
    // Disable console color, you don't need console color when writing the logs to file.
    gin.DisableConsoleColor()

    // Use a file for logging
    f, _ := os.Create("/root/gin.log")
    gin.DefaultWriter = io.MultiWriter(f, os.Stdout)

    r := gin.Default()
    r.GET("/ping", func(c *gin.Context) {
        c.JSON(200, gin.H{
            "message": "pong",
        })
    })
    // Listen and serve on 0.0.0.0:8090
    r.Run(":8090")
}
