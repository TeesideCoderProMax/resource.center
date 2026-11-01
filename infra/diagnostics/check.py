"""Disposable foundation checks. No Resend calls or business messages."""
import json
import os
import uuid

import psycopg
from psycopg import sql
from azure.identity import ManagedIdentityCredential
from azure.cosmos import CosmosClient, exceptions
from azure.servicebus import ServiceBusClient, ServiceBusMessage

mode = os.environ['CHECK_MODE']
credential = ManagedIdentityCredential(client_id=os.environ['CLIENT_ID'])

def record(check, result='PASS'):
    print(json.dumps({'mode': mode, 'check': check, 'result': result}), flush=True)

def postgres(database='loan'):
    return psycopg.connect(
        host=os.environ['PGHOST'], dbname=database, user=os.environ['PGUSER'],
        password=credential.get_token('https://ossrdbms-aad.database.windows.net/.default').token,
        sslmode='verify-full', sslrootcert='/etc/ssl/certs/ca-certificates.crt',
        connect_timeout=20,
    )

def cosmos():
    return CosmosClient(os.environ['COSMOS_ENDPOINT'], credential=credential).get_database_client(
        'notifications').get_container_client('jobs')

def vault_status():
    import requests
    token = credential.get_token('https://vault.azure.net/.default').token
    response = requests.get(os.environ['VAULT_URI'] + 'secrets/foundation-absent-check?api-version=7.4',
                            headers={'Authorization':'Bearer ' + token}, timeout=20)
    return response.status_code

def bus():
    return ServiceBusClient(os.environ['BUS_NAMESPACE'], credential=credential)

if mode == 'bootstrap':
    identities = json.loads(os.environ['IDENTITIES_JSON'])
    with postgres('postgres') as connection:
        for name in ('loan', 'publisher', 'migrations'):
            identity = identities[name]
            exists = connection.execute('SELECT 1 FROM pg_roles WHERE rolname = %s', (identity['name'],)).fetchone()
            if not exists:
                connection.execute('SELECT * FROM pgaadauth_create_principal_with_oid(%s,%s,%s,false,false)',
                                   (identity['name'], identity['principal_id'], 'service'))
    with postgres() as connection:
        loan, publisher, migration = [sql.Identifier(identities[k]['name']) for k in ('loan','publisher','migrations')]
        connection.execute('REVOKE ALL ON DATABASE loan FROM PUBLIC')
        connection.execute('REVOKE CREATE ON SCHEMA public FROM PUBLIC')
        connection.execute(sql.SQL('GRANT CONNECT ON DATABASE loan TO {}, {}, {}').format(loan,publisher,migration))
        connection.execute(sql.SQL('CREATE SCHEMA IF NOT EXISTS loan AUTHORIZATION {}').format(migration))
        connection.execute(sql.SQL('ALTER SCHEMA loan OWNER TO {}').format(migration))
        connection.execute(sql.SQL('GRANT USAGE ON SCHEMA loan TO {}, {}').format(loan,publisher))
        connection.execute(sql.SQL('ALTER DEFAULT PRIVILEGES FOR ROLE {} IN SCHEMA loan GRANT SELECT,INSERT,UPDATE,DELETE ON TABLES TO {}').format(migration,loan))
        connection.execute(sql.SQL('ALTER DEFAULT PRIVILEGES FOR ROLE {} IN SCHEMA loan GRANT USAGE,SELECT ON SEQUENCES TO {}').format(migration,loan))
    record('initialize-role-boundaries')
elif mode == 'migrations':
    # Disposable table is removed by the cleanup mode after Loan DML checks.
    with postgres() as connection:
        connection.execute('CREATE TABLE loan.foundation_diagnostic (id integer PRIMARY KEY)')
    record('postgres-private-tls-migration-ddl')
elif mode == 'cleanup':
    with postgres() as connection:
        connection.execute('DROP TABLE loan.foundation_diagnostic')
    record('diagnostic-table-cleanup')
elif mode in ('loan', 'publisher'):
    assert vault_status() == 403
    record('notification-vault-denied')
    with postgres() as connection:
        connection.execute('SELECT 1')
        record('postgres-private-tls-login')
        assert connection.execute("SELECT has_schema_privilege(current_user, 'loan', 'USAGE')").fetchone()[0]
        assert not connection.execute("SELECT has_schema_privilege(current_user, 'loan', 'CREATE')").fetchone()[0]
        assert not connection.execute("SELECT has_database_privilege(current_user, 'loan', 'CREATE')").fetchone()[0]
        record('postgres-no-ddl')
        if mode == 'loan':
            connection.execute('INSERT INTO loan.foundation_diagnostic VALUES (1)')
            assert connection.execute('SELECT id FROM loan.foundation_diagnostic').fetchone()[0] == 1
            connection.execute('UPDATE loan.foundation_diagnostic SET id = 2 WHERE id = 1')
            connection.execute('DELETE FROM loan.foundation_diagnostic WHERE id = 2')
            record('postgres-loan-dml')
    try:
        cosmos().read_item('foundation-denied', partition_key='foundation-denied')
    except exceptions.CosmosHttpResponseError as exc:
        assert exc.status_code in (401,403), f'Unexpected Cosmos status {exc.status_code}'
        record('cosmos-denied')
    else:
        raise AssertionError('Unexpected Cosmos permission')
    if mode == 'publisher':
        with bus() as client:
            with client.get_queue_sender(os.environ['QUEUE']) as sender:
                sender.send_messages(ServiceBusMessage('foundation diagnostic: do not email',
                                     subject='FoundationDiagnostic', message_id=os.environ['RUN_ID']))
        record('queue-send')
        from azure.servicebus.exceptions import ServiceBusAuthorizationError
        try:
            with bus() as client:
                with client.get_queue_receiver(os.environ['QUEUE'], max_wait_time=5) as receiver:
                    receiver.peek_messages(max_message_count=1)
        except ServiceBusAuthorizationError:
            record('queue-receive-denied')
        else:
            raise AssertionError('Publisher can receive')
elif mode == 'notifications':
    assert vault_status() == 404  # Authorized lookup, deliberately no real secret.
    record('notification-vault-authorized-absent-secret')
    container = cosmos()
    event = 'foundation-' + uuid.uuid4().hex
    body = {'id':event, 'eventId':event, 'kind':'FoundationDiagnostic'}
    container.create_item(body)
    try:
        try:
            container.create_item(body)
        except exceptions.CosmosResourceExistsError:
            record('cosmos-duplicate-conflict')
        else:
            raise AssertionError('Duplicate creation unexpectedly succeeded')
        stored = container.read_item(event, partition_key=event)
        from azure.core import MatchConditions
        container.replace_item(event, {**body, 'checked':True}, etag=stored['_etag'],
                               match_condition=MatchConditions.IfNotModified)
        try:
            container.replace_item(event, body, etag=stored['_etag'],
                                   match_condition=MatchConditions.IfNotModified)
        except exceptions.CosmosHttpResponseError as exc:
            assert exc.status_code == 412
            record('cosmos-stale-etag-denied')
        else:
            raise AssertionError('Stale ETag accepted')
    finally:
        container.delete_item(event, partition_key=event)
    from azure.servicebus.exceptions import ServiceBusAuthorizationError
    try:
        with bus() as client:
            with client.get_queue_sender(os.environ['QUEUE']) as sender:
                sender.send_messages(ServiceBusMessage('foundation-denied', subject='FoundationDiagnostic'))
    except ServiceBusAuthorizationError:
        record('queue-send-denied')
    else:
        raise AssertionError('Notifications can send')
    received = False
    with bus() as client:
        with client.get_queue_receiver(os.environ['QUEUE'], max_wait_time=20) as receiver:
            for message in receiver.receive_messages(max_message_count=10, max_wait_time=20):
                if str(message.message_id) == os.environ['RUN_ID']:
                    receiver.complete_message(message)
                    received = True
                else:
                    receiver.abandon_message(message)
    assert received, 'Diagnostic message was not received'
    record('queue-receive-settle')
    try:
        with postgres():
            pass
    except psycopg.OperationalError as exc:
        # Reject network/TLS failures as evidence of authorization denial.
        assert any(t in str(exc).lower() for t in ('authentication failed', 'role', 'not exist', 'not a member'))
        record('postgres-auth-denied')
    else:
        raise AssertionError('Notifications can authenticate to PostgreSQL')
else:
    raise ValueError('Unknown CHECK_MODE')
credential.close()
record('complete')
