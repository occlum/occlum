import json, cv2, base64
from google.protobuf import json_format


def dict_to_json_msg(data):
    return json.dumps(data)

def json_msg_to_dict(json_msg):
    return json.loads(json_msg)

def proto_msg_to_json_msg(proto_data):
    return json_format.MessageToJson(proto_data)

def proto_msg_to_dict(proto_data):
    return json_msg_to_dict(proto_msg_to_json_msg(proto_data))

def img_to_array(img_path):
    img = cv2.imread(img_path)
    return img

def img_array_to_base64(image_array):
    base64_str = base64.b64encode(image_array).decode('utf-8')
    return base64_str

def base64_to_img_array(base64_str):
    imgString = base64.b64decode(base64_str)
    nparr = np.fromstring(imgString, np.uint8)
    image = cv2.imdecode(nparr, cv2.IMREAD_COLOR)
    return image