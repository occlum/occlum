import tensorflow as tf
mnist = tf.keras.datasets.mnist

# Load and prepare the MNIST dataset. Convert the samples from integers to floating-point numbers
(x_train, y_train), (x_test, y_test) = mnist.load_data(path='/bin/mnist.npz')
x_train, x_test = x_train / 255.0, x_test / 255.0

# Build the tf.keras.Sequential model by stacking layers. Choose an optimizer and loss function for training
model = tf.keras.models.Sequential([
  tf.keras.layers.Flatten(input_shape=(28, 28)),
  tf.keras.layers.Dense(128, activation='relu'),
  tf.keras.layers.Dropout(0.2),
  tf.keras.layers.Dense(10, activation='softmax')
])

model.compile(optimizer='adam',
              loss='sparse_categorical_crossentropy',
              metrics=['accuracy'])

# Train and evaluate model
model.fit(x_train, y_train, epochs=5)

model.evaluate(x_test,  y_test, verbose=2)
