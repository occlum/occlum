package main

import (
	"context"
	"fmt"
	"net"
	"time"

	"google.golang.org/grpc"

	pingpong "grpc_pingpong/github.com/occlum/demos/grpc_pingpong/pingpong"
)

type PingPongServer struct {
        pingpong.UnimplementedPingPongServiceServer
}

func (s *PingPongServer) PingPong(ctx context.Context, in *pingpong.PingPongMesg) (*pingpong.PingPongMesg, error) {
	currentTime := time.Now()
	fmt.Printf("Receiving Ping: %s (at %s)\n", in.Ping, currentTime.Format("2006-01-02T15:04:05.999999999Z07:00"))
	return &pingpong.PingPongMesg{Ping: in.Ping,
				      Pong: fmt.Sprintf("Greetings from Pong! Ping Echoed: %s", in.Ping),
				      Timestamp: currentTime.Format("2006-01-02T15:04:05.999999999Z07:00")}, nil
}

func main() {
	fmt.Println("grpc_pingpong server is waiting for service requests ...")

	conn, err := net.Listen("tcp", "localhost:8888")
	if err != nil {
		fmt.Printf("Failed to listen: %v\n", err)
	}

	grpcServer := grpc.NewServer()

	pingpong.RegisterPingPongServiceServer(grpcServer, &PingPongServer{})

	if err := grpcServer.Serve(conn); err != nil {
		fmt.Printf("Failed to serve: %s\n", err)
	}
}

