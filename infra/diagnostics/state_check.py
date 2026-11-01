"""Run as the workload deployer: verify its state container permissions."""
import email.utils
import json
import os
import uuid
import requests
from azure.identity import ManagedIdentityCredential

credential = ManagedIdentityCredential(client_id=os.environ['CLIENT_ID'])
headers = {
    'Authorization':'Bearer ' + credential.get_token('https://storage.azure.com/.default').token,
    'x-ms-version':'2023-11-03','x-ms-date':email.utils.formatdate(usegmt=True),
}
base = 'https://' + os.environ['STATE_ACCOUNT'] + '.blob.core.windows.net/'
name = 'foundation-access-' + uuid.uuid4().hex
url = base + 'workloads/' + name
response = requests.put(url, data=b'foundation-only', headers={**headers,'x-ms-blob-type':'BlockBlob'}, timeout=20)
assert response.status_code == 201, f'Workload-state upload status {response.status_code}'
try:
    response = requests.get(url, headers=headers, timeout=20)
    assert response.status_code == 200 and response.content == b'foundation-only'
    for container in ('bootstrap','platform'):
        response = requests.get(base + container + '/test/' + container + '.tfstate', headers=headers, timeout=20)
        assert response.status_code == 403, f'Unexpected {container} state status {response.status_code}'
        print(json.dumps({'mode':'deploy','check':container+'-state-denied','result':'PASS'}),flush=True)
finally:
    response = requests.delete(url, headers=headers, timeout=20)
    assert response.status_code == 202
print(json.dumps({'mode':'deploy','check':'own-state-read-write-delete','result':'PASS'}),flush=True)
credential.close()
