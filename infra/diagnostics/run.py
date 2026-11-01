"""Run/delete a disposable no-email foundation job using exported platform outputs."""
import argparse
import json
import re
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['run','status','logs','delete'])
parser.add_argument('mode', choices=['bootstrap','migrations','loan','publisher','notifications','cleanup','deploy'])
args = parser.parse_args()
outputs = Path(__file__).resolve().parents[1] / 'outputs'
handoff = json.loads((outputs / 'test-handoff.json').read_text())
name = 'rc-test-check-' + args.mode
group = handoff['workload_resource_group']
base = ['az','containerapp','job']

def az(arguments):
    result = subprocess.run(arguments, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return result.stdout

if args.action == 'run':
    identity = (json.loads((outputs / 'test-diagnostic-admin.json').read_text()) if args.mode == 'bootstrap'
                else handoff['identities']['migrations' if args.mode in ('cleanup','deploy') else args.mode])
    if args.mode == 'deploy':
        identity = json.loads((outputs/'test-deployer.json').read_text())
    variables = {
        'CHECK_MODE':args.mode, 'CLIENT_ID':identity['client_id'],
        'PGHOST':handoff['postgres_host'], 'PGUSER':identity['name'],
        'COSMOS_ENDPOINT':handoff['cosmos_endpoint'], 'VAULT_URI':handoff['notifications_vault_uri'],
        'BUS_NAMESPACE':handoff['servicebus_namespace'], 'QUEUE':handoff['servicebus_queue'],
        'RUN_ID':(outputs/'test-diagnostic-run-id.txt').read_text().strip(),
        'IDENTITIES_JSON':json.dumps(handoff['identities']),
    }
    job = {
        'location':handoff['location'],
        'identity':{'type':'UserAssigned','userAssignedIdentities':{identity['id']: {}}},
        'tags':{'project':'resource-center','purpose':'temporary-foundation-check'},
        'properties':{
            'environmentId':handoff['container_app_environment_id'], 'workloadProfileName':'Consumption',
            'configuration':{
                'triggerType':'Manual','replicaTimeout':180,'replicaRetryLimit':0,
                'manualTriggerConfig':{'parallelism':1,'replicaCompletionCount':1},
                'registries':[{'server':handoff['registry_login_server'],'identity':identity['id']}],
            },
            'template':{'containers':[{
                'name':'foundation-check', 'image':(outputs/'test-diagnostic-image.txt').read_text().strip(),
                'resources':{'cpu':0.25,'memory':'0.5Gi'},
                'env':[{'name':k,'value':v} for k,v in variables.items()],
            }]},
        },
    }
    if args.mode == 'deploy':
        account = re.search(r'storage_account_name\s*=\s*"([^"]+)"', (outputs.parent/'environments/test/platform.backend.hcl').read_text()).group(1)
        container = job['properties']['template']['containers'][0]
        container['env'].append({'name':'STATE_ACCOUNT','value':account})
        container['command'] = ['python','-c', (Path(__file__).parent/'state_check.py').read_text()]
    payload = outputs / (name + '.json')
    payload.write_text(json.dumps(job))
    url = ('https://management.azure.com/subscriptions/' + handoff['subscription_id'] +
           '/resourceGroups/' + group + '/providers/Microsoft.App/jobs/' + name + '?api-version=2025-01-01')
    az(['az','rest','--method','put','--url',url,'--body','@'+str(payload),'--output','none'])
    print(az(base+['start','-g',group,'-n',name,'--query','name','-o','tsv']).strip())
elif args.action == 'status':
    print(az(base+['execution','list','-g',group,'-n',name,'--query','[].{name:name,status:properties.status}','-o','json']))
elif args.action == 'logs':
    # CLI defaults to the latest execution/replica for a completed single-replica job.
    print(az(base+['logs','show','-g',group,'-n',name,'--container','foundation-check','--tail','100']))
elif args.action == 'delete':
    az(base+['delete','-g',group,'-n',name,'--yes','--output','none'])
    print('Deleted '+name)
