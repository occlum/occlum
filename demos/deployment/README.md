# Docker Image for Deployment

For deployment purpose, we would like to see the image as small as possible. However, Occlum has a variaty of dependencies which is not friendly if users want to deploy the application. 

With the help of [docker multistage build](https://docs.docker.com/develop/develop-images/multistage-build/) and `occlum package` command, we provide dockerfile templates to build a image with the smallest size for deployment environment.

Checkout the dockerfile templates for [Ubuntu](./Dockerfile_template.ubuntu18.04) and [CentOS](./Dockerfile_template.centos8.2). There are three stages in each dockerfile:

    - base stage: This stage configures the package management systems of specific OS and intall required packages for deployment, including `occlum-runtime` and sgx-psw packages. If users want to install specific version of packages, modification should be done in this stage.

    - packager stage: This stage is to build and package the application for deployment. User should also finish the enclave signing in this stage.Users can build your own applications and put to occlum instance. And then use "occlum build" and "occlum package" commands to get a minimum subset of files to run in deployment environment. To support full Occlum commands, extra dependencies are installed.

    - deployer stage: This stage directly inherits environment from "base stage" and unpack the package from "builder stage".

Users can run a quick test with `./deploy_image_test.sh <ubuntu18.04/centos8.2>`.

For different platform, users should modify the `DEVICE_OPTION` variable in the [script](./deploy_image_test.sh) accordingly.
