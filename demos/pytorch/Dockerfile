# Based on https://github.com/petronetto/alpine-machine-learning-base/blob/master/Dockerfile
# and      https://github.com/petronetto/pytorch-alpine/blob/master/Dockerfile

# BSD 3-Clause License
#
# Copyright (c) 2017, Juliano Petronetto
# All rights reserved.
#
# Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions are met:
#
# * Redistributions of source code must retain the above copyright notice, this
#   list of conditions and the following disclaimer.
#
# * Redistributions in binary form must reproduce the above copyright notice,
#   this list of conditions and the following disclaimer in the documentation
#   and/or other materials provided with the distribution.
#
# * Neither the name of the copyright holder nor the names of its
#   contributors may be used to endorse or promote products derived from
#   this software without specific prior written permission.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
# AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
# IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
# DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
# FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
# DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
# SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
# CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
# OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
# OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

FROM alpine:3.10

RUN echo "|--> Updating" \
    && apk update && apk upgrade \
    && echo http://dl-cdn.alpinelinux.org/alpine/v3.10/main | tee /etc/apk/repositories \
    && echo http://dl-cdn.alpinelinux.org/alpine/v3.10/community | tee -a /etc/apk/repositories \
    && echo "|--> Install basics pre-requisites" \
    && apk add --no-cache \
        curl ca-certificates python3 py3-numpy py3-numpy-f2py \
        freetype jpeg libpng libstdc++ libgomp graphviz font-noto \
    && echo "|--> Install Python basics" \
    && python3 -m ensurepip \
    && rm -r /usr/lib/python*/ensurepip \
    && pip3 --no-cache-dir install --upgrade pip setuptools wheel \
    && if [ ! -e /usr/bin/pip ]; then ln -s pip3 /usr/bin/pip; fi \
    && if [[ ! -e /usr/bin/python ]]; then ln -sf /usr/bin/python3 /usr/bin/python; fi \
    && ln -s locale.h /usr/include/xlocale.h \
    && echo "|--> Install build dependencies" \
    && apk add --no-cache --virtual=.build-deps \
        build-base linux-headers python3-dev git cmake jpeg-dev bash \
        libffi-dev gfortran py-numpy-dev freetype-dev libpng-dev \
    && echo "|--> Install Python packages" \
    && pip install -U --no-cache-dir pyyaml cffi requests pillow

# 1) PyTorch is not officially supported on Alpine.
# See https://discuss.pytorch.org/t/compiling-master-from-source-on-alpine-fails-with-undefined-reference-to-backtrace/64676.
# By defining __EMSCRIPTEN__ the build disables certain features like backtrace support.
# This is a work-around.

# 2) Using OpenBLAS caused a segfault. Not installing libopenblas-dev causes PyTorch
# to fall-back to Eigen. Setting BLAS=Eigen does not work due to a bug in the CMake
# script.

# 3) By default, PyTorch uses OpenMP, however due to https://github.com/lsds/sgx-lkl-oe/issues/264
# we need to disable that. Using USE_OPENMP=0 is broken as PyTorch still uses OMP symbols
# but now doesn't link against OpenMP, caused symbol errors.
# Adding ATEN_THREADING=NATIVE and USE_MKLDNN=0 seems to work around the issue.

RUN echo "|--> Install PyTorch" \
    && git clone https://github.com/pytorch/pytorch \
    && cd pytorch \
    && git checkout v1.4.1 \
    && git submodule update --init --recursive \
    && DEBUG=0 USE_CUDA=0 USE_MKLDNN=0 USE_OPENMP=0 ATEN_THREADING=NATIVE BUILD_BINARY=0 \
       CFLAGS="-D__EMSCRIPTEN__" \
       python setup.py install \
    && cd .. \
    && rm -rf pytorch

ADD app app
