# Flask TLS demo on Occlum

This project demonstrates how Occlum enables _unmodified_ [Python](https://www.python.org) program [`flask`](https://github.com/pallets/flask) running in SGX enclaves, which is based on glibc.

`Flask` is a lightweight WSGI web application framework. It is designed to make getting started quick and easy, with the ability to scale up to complex applications.

## Sample Code: Flask TLS demo in Python

To make the sample code more realistic, we choose to start a simple Flask TLS server by [`flask-restful`](https://flask-restful.readthedocs.io/en/latest/quickstart.html). The sample code can be found [here](rest_api.py).

## How to Run

This tutorial is written under the assumption that you have Docker installed and use Occlum in a Docker container.

* Step 1: Download miniconda and install python to prefix position.
```
bash ./install_python_with_conda.sh
```

* Step 2: Generate sample cert/key
```
bash ./gen-cert.sh
```

* Step 3: Build Flask TLS Occlum instance
```
bash ./build_occlum_instance.sh
```

* Step 4: Start the Flask TLS server on Occlum
```
bash ./run_flask_on_occlum.sh
```
It starts a sample Flask server like below:
```
occlum run /bin/rest_api.py
 * Serving Flask app "rest_api" (lazy loading)
 * Environment: production
   WARNING: This is a development server. Do not use it in a production deployment.
   Use a production WSGI server instead.
 * Debug mode: off
 * Running on all addresses.
   WARNING: This is a development server. Do not use it in a production deployment.
 * Running on https://localhost:4996/ (Press CTRL+C to quit)
 ```

* Step 5: Write some customers' info, such as
```
# curl --cacert flask.crt -X PUT https://localhost:4996/customer/1 -d "data=Tom"
# curl --cacert flask.crt -X PUT https://localhost:4996/customer/2 -d "data=Jerry"
```

* Step 6: Read the customers' info back
```
# curl --cacert flask.crt -X GET https://localhost:4996/customer/1
# curl --cacert flask.crt -X GET https://localhost:4996/customer/2
```