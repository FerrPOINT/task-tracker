"""Fixed CI/service flags from the canonical Base plan; never runs a producer."""
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts.vendor import base_quality_selection as selector


def describe(plan):
    gates = {gate['id'] for gate in plan['gates']}
    return {
        'backend': bool(set(plan['components']) & {'backend', 'cli'}),
        'server_checks': 'backend' in plan['components'],
        'frontend': 'frontend' in plan['components'],
        'backend_build': 'backend:build' in gates,
        'cli_build': 'cli:build' in gates,
        'frontend_build': 'frontend:build' in gates,
        'openapi_check': 'backend:build' in gates or any(
            selector.matches(name, ['openapi/**']) for name in plan['reasons'].get('backend', [])),
    }


def build_services(plan):
    values = describe(plan)
    # Binary/Vite producers and container recipes have different inputs.
    # A changed recipe must still package its service without inventing an app build.
    inputs = selector.load(ROOT).get('container_inputs', {})
    return [name for name in ('backend', 'frontend') if values[name + '_build'] or any(
        selector.matches(path, inputs.get(name, [])) for path in plan['changed'])]


def auto_services(root=ROOT):
    return build_services(selector.plan(root, profile='build', base='HEAD'))


def outputs(root, event, base, head):
    manifest = selector.load(root)
    if event not in ('pull_request', 'push') or not re.fullmatch('[0-9a-f]{40}', head) or head == '0' * 40:
        raise ValueError('Choose a supported event and exact candidate head')
    if event == 'push' and base == '0' * 40:
        plan = selector.plan(root, profile='build', changed=[], components=list(manifest['components']), manifest=manifest)
    else:
        if not re.fullmatch('[0-9a-f]{40}', base) or base == '0' * 40:
            raise ValueError('Choose an exact comparison base before preparation')
        paths = selector.git(root, 'diff', '--name-only', '--no-renames', base, head).splitlines()
        plan = selector.plan(root, profile='build', changed=paths, manifest=manifest)
    return describe(plan)


if __name__ == '__main__':
    values = outputs(ROOT, *(os.environ.get(name, '') for name in ('EVENT_NAME', 'BASE_SHA', 'HEAD_SHA')))
    text = ''.join(name + '=' + str(value).lower() + '\n' for name, value in values.items())
    if os.environ.get('GITHUB_OUTPUT'):
        with Path(os.environ['GITHUB_OUTPUT']).open('a', encoding='utf-8') as stream:
            stream.write(text)
    else:
        print(text, end='')
