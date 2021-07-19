#
# -*- coding: utf-8 -*-
#
# Copyright (c) 2019 Intel Corporation
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# SPDX-License-Identifier: EPL-2.0
#

"""Import a model graph and export a SavedModel.

Usage: model_graph_to_saved_model.py [--model_version=y] import_path export_dir
"""

from __future__ import print_function

import sys
from collections import OrderedDict
import tensorflow.compat.v1 as tf
from tensorflow.python.tools.optimize_for_inference_lib import optimize_for_inference
from tensorflow.python.framework import dtypes


INPUTS = 'input'
OUTPUTS = 'predict'

tf.app.flags.DEFINE_integer('model_version', 1, 'Version number of the model.')
tf.app.flags.DEFINE_string('import_path', '', 'Model import path.')
tf.app.flags.DEFINE_string('export_dir', '/tmp', 'Export directory.')
tf.app.flags.DEFINE_string('inputs', INPUTS, 'Export directory.')
tf.app.flags.DEFINE_string('outputs', OUTPUTS, 'Export directory.')
tf.app.flags.DEFINE_string('dtypes', 'float32', 'Export directory.')
FLAGS = tf.app.flags.FLAGS


def main(_):
    if len(sys.argv) < 2 or sys.argv[-1].startswith('-'):
        print('Usage: model_graph_to_saved_model.py [--model_version=y] import_path export_dir')
        sys.exit(-1)
    if FLAGS.import_path == '':
        print('Please specify the path to the model graph you want to convert to SavedModel format.')
        sys.exit(-1)
    if FLAGS.model_version <= 0:
        print('Please specify a positive value for version number.')
        sys.exit(-1)

    # Import model graph
    with tf.Session(config=tf.ConfigProto(allow_soft_placement=True, log_device_placement=True)) as sess:
        graph_def = tf.GraphDef()
        with tf.gfile.GFile(FLAGS.import_path, 'rb') as input_file:
            input_graph_content = input_file.read()
            graph_def.ParseFromString(input_graph_content)

        # Apply transform optimizations
        # https://www.tensorflow.org/api_docs/python/tf/dtypes/DType
        output_graph = optimize_for_inference(graph_def, [FLAGS.inputs], [FLAGS.outputs], dtypes.float32.as_datatype_enum, True)
        # output_graph = graph_def

        sess.graph.as_default()
        tf.import_graph_def(output_graph, name='')
        # print(sess.graph.get_operations())

        # Replace the signature_def_map.
        in_image = sess.graph.get_tensor_by_name(FLAGS.inputs + ':0')
        inputs = {INPUTS: tf.compat.v1.saved_model.build_tensor_info(in_image)}

        out_classes = sess.graph.get_tensor_by_name(FLAGS.outputs + ':0')
        outputs = {OUTPUTS: tf.compat.v1.saved_model.build_tensor_info(out_classes)}

        signature = tf.saved_model.signature_def_utils.build_signature_def(
            inputs=inputs,
            outputs=outputs,
            method_name=tf.saved_model.signature_constants.PREDICT_METHOD_NAME
        )

        # Save out the SavedModel
        print('Exporting trained model to', FLAGS.export_dir + '/' + str(FLAGS.model_version))
        builder = tf.saved_model.builder.SavedModelBuilder(FLAGS.export_dir + '/' + str(FLAGS.model_version))
        builder.add_meta_graph_and_variables(
            sess, [tf.saved_model.tag_constants.SERVING],
            signature_def_map={
                tf.saved_model.signature_constants.DEFAULT_SERVING_SIGNATURE_DEF_KEY: signature
            }
        )
        builder.save()

    print('Done!')


if __name__ == '__main__':
    tf.app.run()
