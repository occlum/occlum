import paddle
import numpy as np
from paddle.vision.transforms import Normalize

transform = Normalize(mean=[127.5], std=[127.5], data_format='CHW')
train_dataset = paddle.vision.datasets.MNIST(image_path='mnist/train-images-idx3-ubyte.gz',
                                             label_path='mnist/train-labels-idx1-ubyte.gz',
                                             mode='train', transform=transform)
test_dataset = paddle.vision.datasets.MNIST(image_path='mnist/t10k-images-idx3-ubyte.gz',
                                            label_path='mnist/t10k-labels-idx1-ubyte.gz',
                                            mode='test', transform=transform)

lenet = paddle.vision.models.LeNet(num_classes=10)
model = paddle.Model(lenet)

model.prepare(paddle.optimizer.Adam(parameters=model.parameters()),
              paddle.nn.CrossEntropyLoss(),
              paddle.metric.Accuracy())

model.fit(train_dataset, epochs=5, batch_size=64, verbose=1)
model.evaluate(test_dataset, batch_size=64, verbose=1)

model.save('./output/mnist')
model.load('output/mnist')

img, label = test_dataset[0]
img_batch = np.expand_dims(img.astype('float32'), axis=0)

out = model.predict_batch(img_batch)[0]
pred_label = out.argmax()
print('true label: {}, pred label: {}'.format(label[0], pred_label))
