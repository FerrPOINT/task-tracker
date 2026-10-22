"""Source contract only; native artifact/freshness/performance need qualification."""
import json
import re
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
PRODUCT = 'task-tracker'
RECIPES = (ROOT / 'backend/Dockerfile', ROOT / 'backend/Dockerfile.umbrella')
SDK = ('sdlc-telemetry', 'sdlc-auth-core', 'sdlc-cli-core', 'sdlc-shared')


def stages(text):
    result = {}
    for part in re.split(r'(?m)^FROM ', text)[1:]:
        first, body = part.split('\n', 1)
        fields = first.split()
        alias = fields[-1] if len(fields) == 3 and fields[1].upper() == 'AS' else 'runtime'
        result[alias] = ({fields[0]} | set(re.findall(r'(?:--from=|\bfrom=)([a-zA-Z0-9_-]+)', body)),
                         body.replace('\\\n', ' '))
    return result


class BackendDependencyLayerTests(unittest.TestCase):
    def test_unit_has_real_sources_and_no_server_dependency_ancestor(self):
        for recipe in RECIPES:
            with self.subTest(recipe=recipe.name):
                graph = stages(recipe.read_text(encoding='utf-8'))
                self.assertTrue({'manifests','unit-dependencies','dependencies','unit','builder'} <= graph.keys())
                seen, pending = set(), ['unit']
                while pending:
                    name = pending.pop()
                    if name in seen or name not in graph:
                        continue
                    seen.add(name)
                    pending.extend(graph[name][0])
                self.assertNotIn('dependencies', seen)
                self.assertFalse(any('cargo build' in graph[name][1] for name in seen))
                unit = graph['unit'][1]
                self.assertIn('cargo test --locked --release -p domain --lib', unit)
                self.assertNotIn('--no-run', unit)
                self.assertIn("printf 'passed", unit)
                for member in ('shared','domain'):
                    self.assertIn('COPY '+PRODUCT+'/backend/'+member+' ./'+member, unit)
                self.assertIn('from=unit,source=/tmp/rust-unit-check', graph['builder'][1])

    def test_metadata_inputs_and_stub_cleanup_cover_actual_workspace_packages(self):
        members = tomllib.loads((ROOT/'backend/Cargo.toml').read_text(encoding='utf-8'))['workspace']['members']
        packages = {tomllib.loads((ROOT/'backend'/name/'Cargo.toml').read_text(encoding='utf-8'))['package']['name'] for name in members}
        for recipe in RECIPES:
            with self.subTest(recipe=recipe.name):
                graph = stages(recipe.read_text(encoding='utf-8'))
                self.assertIn('manifests', graph)
                for member in members:
                    self.assertIn('COPY '+PRODUCT+'/backend/'+member+'/Cargo.toml ', graph['manifests'][1])
                for name in ('manifests','unit-dependencies','dependencies'):
                    for line in graph[name][1].splitlines():
                        if line.startswith('COPY '):
                            for source in line.split()[1:-1]:
                                if source.startswith(PRODUCT+'/backend'):
                                    self.assertTrue(source.endswith(('Cargo.toml','Cargo.lock*')), line)
                for name in ('unit-dependencies','dependencies'):
                    body = graph[name][1]
                    self.assertIn('cargo clean --release', body)
                    self.assertEqual(set(re.findall(r'-p ([a-zA-Z0-9_-]+)',body.split('cargo clean',1)[-1])),packages)
                    self.assertNotIn('rust-unit-check',body)

    def test_download_cache_is_shared_but_compiled_target_is_not_mutable(self):
        for recipe in RECIPES:
            with self.subTest(recipe=recipe.name):
                text=recipe.read_text(encoding='utf-8').replace('\\\n',' ')
                self.assertIn('ARG CARGO_BUILD_JOBS=2',text)
                self.assertIn('CARGO_REGISTRY_CACHE_ID=pdlc-operator-downloads-v1-cargo188',text)
                self.assertIn('id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH}',text)
                self.assertNotIn('CARGO_TARGET_DIR=',text)
                for mount in re.findall(r'--mount=type=cache,([^\s]+)',text):
                    target=re.search(r'(?:^|,)target=([^,]+)',mount)[1]
                    self.assertIn(target,('/usr/local/cargo/registry','/.__pdlc_build_slot__'))
                for line in text.splitlines():
                    if line.startswith('RUN ') and 'cargo ' in line:
                        self.assertIn('target=/.__pdlc_build_slot__',line)

    def test_context_is_product_and_original_sdk_only_with_private_exclusions(self):
        allowed={'!'+PRODUCT+'/','!'+PRODUCT+'/backend/','!'+PRODUCT+'/backend/**',
                 '!services-base/','!services-base/Cargo.toml','!services-base/crates/'}
        for name in SDK:
            allowed.update(('!services-base/crates/'+name+'/','!services-base/crates/'+name+'/**'))
        for recipe in RECIPES:
            with self.subTest(recipe=recipe.name):
                path=recipe.with_name(recipe.name+'.dockerignore')
                self.assertTrue(path.exists())
                rules=[line for line in path.read_text(encoding='utf-8').splitlines() if line and not line.startswith('#')]
                self.assertEqual(rules[0],'**')
                for barrier in (PRODUCT+'/*','services-base/*','services-base/crates/*'):
                    self.assertIn(barrier,rules)
                positives=[line for line in rules if line.startswith('!')]
                self.assertEqual(set(positives),allowed)
                self.assertEqual(len(positives),len(allowed),'No later duplicate parent exception')
                for parent,barrier,child in (
                    ('!'+PRODUCT+'/',PRODUCT+'/*','!'+PRODUCT+'/backend/'),
                    ('!services-base/','services-base/*','!services-base/Cargo.toml'),
                    ('!services-base/crates/','services-base/crates/*','!services-base/crates/'+SDK[0]+'/')):
                    self.assertLess(rules.index(parent),rules.index(barrier))
                    self.assertLess(rules.index(barrier),rules.index(child))
                for name in SDK:
                    self.assertGreater(rules.index('!services-base/crates/'+name+'/'),rules.index('services-base/crates/*'))
                last_exception=max(rules.index(line) for line in positives)
                for token in ('**/.git','**/.local','**/.env','**/.env.*','**/target','**/node_modules','**/coverage','**/test-results'):
                    self.assertIn(token,rules)
                    self.assertGreater(rules.index(token),last_exception)

    def test_sidecar_preserves_live_workspace_private_exclusions(self):
        policy=ROOT.parent/'.dockerignore'
        if not (ROOT.parent/'source-workspace.json').is_file() or not policy.is_file():
            self.skipTest('Standalone checkout: no PDLC workspace private policy')
        source=policy.read_text(encoding='utf-8')
        self.assertIn('**/.git',source)
        private=['**/.git',*source.split('**/.git',1)[1].splitlines()]
        private=[line.strip() for line in private if line.strip() and not line.startswith('#')]
        for recipe in RECIPES:
            rules=recipe.with_name(recipe.name+'.dockerignore').read_text(encoding='utf-8').splitlines()
            last_exception=max(i for i,line in enumerate(rules) if line.startswith('!'))
            for rule in private:
                self.assertIn(rule,rules)
                self.assertGreater(rules.index(rule),last_exception)

    def test_server_consumers_runtime_and_single_guard_owner_survive(self):
        packages={tomllib.loads(p.read_text(encoding='utf-8'))['package']['name']:p.parent.name for p in (ROOT/'backend').glob('*/Cargo.toml')}
        seen,pending=set(),['server']
        while pending:
            name=pending.pop()
            if name in seen or name not in packages:
                continue
            seen.add(name)
            value=tomllib.loads((ROOT/'backend'/packages[name]/'Cargo.toml').read_text(encoding='utf-8'))
            pending.extend(value.get('dependencies',{}))
        for recipe in RECIPES:
            with self.subTest(recipe=recipe.name):
                text=recipe.read_text(encoding='utf-8');graph=stages(text)
                self.assertIn('builder',graph)
                for name in seen:
                    member=packages[name]
                    self.assertIn('COPY '+PRODUCT+'/backend/'+member+' ./'+member,graph['builder'][1])
                self.assertIn('cargo build --locked --release -p server --bin server',graph['builder'][1])
                for crate in SDK:
                    self.assertIn('COPY services-base/crates/'+crate+' /services-base/crates/'+crate,text)
                self.assertIn('USER tasktracker',graph['runtime'][1]); self.assertIn('ENV TASKTRACKER_UPLOADS_DIR=/var/lib/tasktracker/uploads',graph['runtime'][1]); self.assertIn('CMD ["task-tracker-server"]',graph['runtime'][1])
        manifest=json.loads((ROOT/'.quality-gate.json').read_bytes())
        module='scripts.tests.test_backend_dependency_layers'
        for profile in ('fast','unit','build','release'):
            owners=[g for g in manifest['gates'] if profile in g['profiles'] and module in g['argv']]
            self.assertEqual(len(owners),1)
            self.assertEqual(owners[0]['execution_slot'],'none')
        self.assertIn(module,(ROOT/'.github/workflows/ci.yml').read_text(encoding='utf-8'))
