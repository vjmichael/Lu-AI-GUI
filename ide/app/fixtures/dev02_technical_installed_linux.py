#!/usr/bin/env python3
"""Real installed GUI registration witness; no provider is launched."""
import hashlib
import os
from pathlib import Path
import subprocess
import wo8_linux_editor_atspi as native


def main():
    if os.environ.get('GITHUB_ACTIONS') != 'true' or not os.environ.get('DISPLAY'):
        raise RuntimeError('Disposable real GUI runner required')
    if native.EXE != '/usr/bin/lu-ide-desktop' or not native.ROOT.is_relative_to(Path(os.environ['RUNNER_TEMP']).resolve()):
        raise RuntimeError('Not the installed native app or disposable project')
    before = hashlib.sha256(native.SOURCE.read_bytes()).hexdigest()
    # An empty baseline is mandatory on the isolated hosted runner.
    for name in ('clangd', 'rust-analyzer', 'ruff', 'lldb-dap'):
        if subprocess.run(['pgrep','-x',name],stdout=subprocess.DEVNULL).returncode != 1:
            raise RuntimeError('Unexpected provider process: '+name)
    app = native.start()
    try:
        native.select_native_folder()
        native.editor_project()
        if hashlib.sha256(native.SOURCE.read_bytes()).hexdigest() != before:
            raise RuntimeError('Read-only source changed')
        for name in ('clangd', 'rust-analyzer', 'ruff', 'lldb-dap'):
            if subprocess.run(['pgrep','-x',name],stdout=subprocess.DEVNULL).returncode != 1:
                raise RuntimeError('Unexpected provider process: '+name)
        print('DEV02_TECHNICAL_INSTALLED_GTK_NATIVE_REGISTRATION=PASS', flush=True)
    finally:
        native.stop(app)

if __name__ == '__main__':
    main()
