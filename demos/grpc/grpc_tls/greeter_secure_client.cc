#include <memory>
#include <string>
#include <sstream>
#include <fstream>
#include <iostream>
#include <grpc++/grpc++.h>

#include "helloworld.grpc.pb.h"

using grpc::Channel;
using grpc::ClientContext;
using grpc::Status;

using helloworld::HelloRequest;
using helloworld::HelloReply;
using helloworld::Greeter;

class GreeterClient
{
public:
    GreeterClient ( const std::string& cert,
                    const std::string& key,
                        const std::string& root,
                        const std::string& server )
    {
        grpc::SslCredentialsOptions opts =
        {
            root,
            key,
            cert
        };

        stub_ = Greeter::NewStub ( grpc::CreateChannel (
            server, grpc::SslCredentials ( opts ) ) );
    }

    std::string
    SayHello ( const std::string& user )
    {
        HelloRequest request;
        request.set_name(user);

        HelloReply reply;

        ClientContext context;

        Status status = stub_->SayHello ( &context, request, &reply );

        if ( status.ok () )
        {
            return reply.message ();
        }
        else
        {
            std::cout << status.error_code () << ": "
                      << status.error_message () << std::endl;
            return "RPC failed";
        }
    }

private:
    std::unique_ptr<Greeter::Stub> stub_;
};

void
read ( const std::string& filename, std::string& data )
{
    std::ifstream file ( filename.c_str (), std::ios::in );

    if ( file.is_open () )
    {
        std::stringstream ss;
        ss << file.rdbuf ();

        file.close ();

        data = ss.str ();
    }

    return;
}

int
main ( int argc, char** argv )
{
    std::string cert;
    std::string key;
    std::string root;
    std::string server { "localhost:50051" };

    read ( "client.crt", cert );
    read ( "client.key", key );
    read ( "ca.crt", root );

    GreeterClient greeter ( cert, key, root, server );

    std::string user ( "world" );
    std::string reply = greeter.SayHello ( user );

    std::cout << "Greeter received: " << reply << std::endl;

    return 0;
}

