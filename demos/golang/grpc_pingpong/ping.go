package main

import (
	"fmt"
	"time"

	"golang.org/x/net/context"
	"google.golang.org/grpc"

	pingpong "grpc_pingpong/github.com/occlum/demos/grpc_pingpong/pingpong"
)

func main() {
	conn, err := grpc.Dial("localhost:8888", grpc.WithInsecure())
	if err != nil {
		fmt.Printf("Failed to connect: %s\n", err)
	}
	defer conn.Close()

	previousTime := time.Now()
	fmt.Printf("Ping: at %s\n", previousTime.Format("2006-01-02T15:04:05.999999999Z07:00"))
	client := pingpong.NewPingPongServiceClient(conn)
	response, err := client.PingPong(context.Background(), &pingpong.PingPongMesg{Ping: "Hello"})
	if err != nil {
		fmt.Printf("Error when calling PingPongHandler: %s\n", err)
	}
	fmt.Printf("Pong from server: %s (at %s)\n", response.Pong, response.Timestamp)
	currentTime := time.Now()
	fmt.Printf("End-to-End latency is: %f Seconds\n", currentTime.Sub(previousTime).Seconds())
}

