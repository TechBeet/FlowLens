import subprocess
import os

os.chdir('d:/github/FlowLens')

result = subprocess.run(['git', 'status', '--porcelain'], capture_output=True, text=True)
changes = result.stdout.strip().splitlines()

committed = 0
for line in changes:
    if not line.strip():
        continue
    status = line[:2]
    path = line[3:].strip()
    if not path:
        continue
    isDeleted = 'D' in status
    if not isDeleted and not os.path.exists(path):
        continue
    # stage the file (addition or deletion)
    subprocess.run(['git', 'add', path], capture_output=True)
    if isDeleted:
        msg = 'delete: ' + path
    else:
        msg = 'add: ' + path
    subprocess.run(['git', 'commit', '-m', msg], capture_output=True)
    print('Committed: ' + msg)
    committed += 1

print('Total commits created: ' + str(committed))
push = subprocess.run(['git', 'push', 'origin', 'main'], capture_output=True, text=True)
print('Push stdout:', push.stdout)
print('Push stderr:', push.stderr)
print('Push returncode:', push.returncode)
