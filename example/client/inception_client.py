import grpc
import tensorflow as tf
import argparse

from tensorflow_serving.apis import predict_pb2
from tensorflow_serving.apis import prediction_service_pb2_grpc


def main():
  with open(args.crt, 'rb') as f:
    creds = grpc.ssl_channel_credentials(f.read())
  channel = grpc.secure_channel(args.server, creds)
  stub = prediction_service_pb2_grpc.PredictionServiceStub(channel)
  # Send request
  with open(args.image, 'rb') as f:
    # See prediction_service.proto for gRPC request/response details.
    request = predict_pb2.PredictRequest()
    request.model_spec.name = 'INCEPTION'
    request.model_spec.signature_name = 'predict_images'

    input_name = 'images'
    input_shape = [1]
    input_data = f.read()
    request.inputs[input_name].CopyFrom(
      tf.make_tensor_proto(input_data, shape=input_shape))

    result = stub.Predict(request, 10.0)  # 10 secs timeout
    print(result)

  print("Inception Client Passed")


if __name__ == '__main__':
  parser = argparse.ArgumentParser()
  parser.add_argument('--server', default='localhost:9000',
                      help='Tenforflow Model Server Address')
  parser.add_argument('--crt', default=None, type=str, help='TLS certificate file path')
  parser.add_argument('--image', default='Siberian_Husky_bi-eyed_Flickr.jpg',
                      help='Path to the image')
  args = parser.parse_args()

  main()