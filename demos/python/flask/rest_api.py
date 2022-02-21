#!/usr/bin/python3

import sys
import os

from flask import Flask, request
from flask_restful import Resource, Api

sys.path.insert(0, os.path.dirname(__file__))

cert = '/etc/flask.crt'
cert_key = '/etc/flask.key'

app = Flask(__name__)
api = Api(app)

customers = {}

class Customer(Resource):
    def get(self, customer_id):
        return {customer_id: customers[customer_id]}

    def put(self, customer_id):
        customers[customer_id] = request.form['data']
        return {customer_id: customers[customer_id]}

api.add_resource(Customer, '/customer/<string:customer_id>')

if __name__ == '__main__':
    app.debug = False
    ssl_context = (cert, cert_key)
    app.run(host='0.0.0.0', port=4996, threaded=True, ssl_context=ssl_context)
    #app.run(host='0.0.0.0', port=4996, threaded=True)
