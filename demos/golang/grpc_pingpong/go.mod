module grpc_pingpong

go 1.18

require (
	golang.org/x/net v0.17.0
	google.golang.org/grpc v1.58.2
)

require (
	github.com/golang/protobuf v1.5.3 // indirect
	golang.org/x/sys v0.13.0 // indirect
	golang.org/x/text v0.13.0 // indirect
	google.golang.org/genproto/googleapis/rpc v0.0.0-20230711160842-782d3b101e98 // indirect
	google.golang.org/protobuf v1.31.0 // indirect
)

replace golang.org/x/sys => golang.org/x/sys v0.31.0
replace google.golang.org/protobuf => google.golang.org/protobuf v1.31.0
