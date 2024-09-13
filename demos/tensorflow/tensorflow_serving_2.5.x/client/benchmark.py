import grpc
import tensorflow as tf
import argparse, time, grpc, asyncio
import numpy as np
from PIL import Image

from tensorflow_serving.apis import predict_pb2
from tensorflow_serving.apis import prediction_service_pb2_grpc


class benchmark_engine(object):
    def __init__(self, server, image, certificate, concurrent_num=64, response_time=10):
        self.server = server
        self.response_time = response_time
        self.concurrent_num = concurrent_num
        self.image = image
        self.certificate = certificate
        self.request_signatures = []
        self.request_stubs = []
        self.request_response_list = {}
        self.__prepare__()
        pass

    def __prepare__(self):
        for idx in range(self.concurrent_num):
            # get image array
            # with open(self.image, 'rb') as f:
            #     input_name = 'images'
            #     input_shape = [1]
            #     input_data = f.read()

            # Load the image and convert to RGB
            img = Image.open(self.image).convert('RGB')
            img = img.resize((224,224), Image.BICUBIC)
            img_array = np.array(img)
            img_array = img_array.astype(np.float32) /255.0
            # create request
            request = predict_pb2.PredictRequest()
            request.model_spec.name = 'resnet'
            request.model_spec.signature_name = 'serving_default'
            request.inputs['input_1'].CopyFrom(
                tf.make_tensor_proto(img_array, shape=[1,224,224,3]))
            
            self.request_signatures.append(request)
        return None

    async def __connection__(self, task_idx, loop_num):
        request_signatures = self.request_signatures[task_idx]
        response_list = []

        # create channel
        creds = grpc.ssl_channel_credentials(root_certificates=open(self.certificate, 'rb').read())
        async with grpc.aio.secure_channel(self.server, creds) as channel:
            stub = prediction_service_pb2_grpc.PredictionServiceStub(channel)
            format_string = 'query: {} channel, task {}, loop_idx {}, latency(ms) {:.1f}, tps: {:.1f}'
            for loop_idx in range(loop_num):
                start_time = time.time()
                response = await stub.Predict(request_signatures)
                stop_time = time.time()
                latency = stop_time - start_time
                tps = 1 / latency
                response_list.append([response, latency])
                print(format_string.format('secure', task_idx, loop_idx, 1000*latency, tps))
        return response_list

    def run(self, loop_num):
        start_time = time.time()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        connections = []
        self.request_response_list.clear()
        for idx in range(self.concurrent_num):
            connections.append(asyncio.ensure_future(self.__connection__(idx, loop_num)))

        loop.run_until_complete(asyncio.wait(connections))
        loop.close()

        stop_time = time.time()

        response_list = [connections[idx].result() for idx in range(self.concurrent_num)]

        request_time = 0
        for c_idx in range(self.concurrent_num):
            if loop_num != 0:
                for l_idx in range(loop_num):
                    request_time += response_list[c_idx][l_idx][1]

        if loop_num != 0:
            e2e_time = stop_time - start_time
            request_num = self.concurrent_num * loop_num
            latency = request_time / request_num
            tps = request_num * 1 / e2e_time
            format_string = 'summary: cnum {}, e2e time(s) {}, average latency(ms) {}, tps: {}'
            print(format_string.format(self.concurrent_num, e2e_time, 1000*latency, tps))
    pass

def main():
    benchmark_app = benchmark_engine(args.server, args.image, args.crt, args.cnum)
    if args.loop == 0:
        print("loop parameter needs to be bigger than 0")
        return

    # warm up
    benchmark_app.run(5)
    # start loop
    benchmark_app.run(args.loop)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', default='localhost:9000',
                        help='Tenforflow Model Server Address')
    parser.add_argument('--crt', default=None, type=str, help='TLS certificate file path')
    parser.add_argument('--image', default='Siberian_Husky_bi-eyed_Flickr.jpg',
                        help='Path to the image')
    parser.add_argument('--cnum', default=8, type=int, help='Concurrent connection num')
    parser.add_argument('--loop', default=100, type=int, help='Requests loop num, should > 0')
    args = parser.parse_args()

    main()