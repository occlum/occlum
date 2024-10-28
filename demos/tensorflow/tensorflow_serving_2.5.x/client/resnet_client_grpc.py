import grpc
import tensorflow as tf
import argparse
import numpy as np
from PIL import Image

from tensorflow_serving.apis import predict_pb2
from tensorflow_serving.apis import prediction_service_pb2_grpc


def main():
  with open(args.crt, 'rb') as f:
    creds = grpc.ssl_channel_credentials(f.read())
  channel = grpc.secure_channel(args.server, creds)
  stub = prediction_service_pb2_grpc.PredictionServiceStub(channel)

  # Load the image and convert to RGB
  img = Image.open(args.image).convert('RGB')
  img = img.resize((224,224), Image.BICUBIC)
  img_array = np.array(img)
  img_array = img_array.astype(np.float32) /255.0

  # Create a request message for TensorFlow Serving
  request = predict_pb2.PredictRequest()
  request.model_spec.name = 'resnet'
  request.model_spec.signature_name = 'serving_default'
  request.inputs['input_1'].CopyFrom(
    tf.make_tensor_proto(img_array, shape=[1,224,224,3]))

  # Send the request to TensorFlow Serving
  result = stub.Predict(request, 25.0)

  # Print the predicted class and probability
  result = result.outputs['activation_49'].float_val
  class_idx = np.argmax(result)
  print('Prediction class: ', class_idx)
  print('Probability: ', result[int(class_idx)])

if __name__ == '__main__':
  parser = argparse.ArgumentParser()
  parser.add_argument('--server', default='localhost:9000',
                      help='Tenforflow Model Server Address')
  parser.add_argument('--crt', default=None, type=str, help='TLS certificate file path')
  parser.add_argument('--image', default='Siberian_Husky_bi-eyed_Flickr.jpg',
                      help='Path to the image')
  args = parser.parse_args()

  main()