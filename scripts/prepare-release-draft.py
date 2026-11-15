"""Attach exact successful main-CI bytes to an existing private release draft.

Never creates a tag, publishes a release, overwrites assets or bypasses signing.
Unsigned publication requires a separate explicit maintainer decision.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


def gh(*args):
    return subprocess.check_output(['gh', *args], text=True)


def api(path):
    return json.loads(gh('api', path))


def main():
    repo = os.environ['GITHUB_REPOSITORY']
    run_id = os.environ['CHECKED_RUN_ID']
    tag = os.environ['DRAFT_TAG']
    if not run_id.isdecimal() or not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise SystemExit('Invalid run ID or version tag')
    version = re.search(r'^version = "([^"]+)"$', Path('Cargo.toml').read_text(), re.M)[1]
    if tag != 'v' + version:
        raise SystemExit('Draft tag differs from Cargo.toml')
    run = api(f'repos/{repo}/actions/runs/{run_id}')
    if (run['conclusion'] != 'success' or run['status'] != 'completed'
            or run['event'] != 'push' or run['head_branch'] != 'main'
            or run['head_repository']['full_name'] != repo
            or run['path'] != '.github/workflows/ci.yml'):
        raise SystemExit('Only successful trusted main CI is accepted')
    commit = run['head_sha']
    if not re.fullmatch('[0-9a-f]{40}', commit):
        raise SystemExit('Invalid CI commit')
    releases = api(f'repos/{repo}/releases?per_page=100')
    matches = [r for r in releases if r['tag_name'] == tag]
    if len(matches) != 1:
        raise SystemExit('Exactly one existing draft is required')
    release_id = matches[0]['id']

    def checked_draft():
        release = api(f'repos/{repo}/releases/{release_id}')
        if (not release['draft'] or not release['prerelease']
                or release['target_commitish'] != commit or release['tag_name'] != tag):
            raise SystemExit('Refusing to change a published or mismatched release')
        return release

    if checked_draft()['assets']:
        raise SystemExit('Draft already contains assets; refusing to overwrite')
    with tempfile.TemporaryDirectory(prefix='hushlink-draft-') as temp:
        root = Path(temp)
        subprocess.run(['gh', 'run', 'download', run_id, '--repo', repo,
                        '--name', f'windows-x64-{commit}', '--dir', str(root / 'checked')], check=True)
        if (root / 'checked/build-commit.txt').read_text(encoding='utf-8-sig').strip() != commit:
            raise SystemExit('Artifact commit mismatch')
        assets = root / 'checked/release-assets'
        verifier = str(Path(__file__).with_name('verify-package.py'))
        subprocess.run([sys.executable, verifier, '--directory', str(assets),
                        '--tag', tag, '--commit', commit], check=True)
        expected = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in assets.iterdir() if p.is_file()}
        for path in sorted(assets.iterdir()):
            checked_draft()
            subprocess.run(['gh', 'release', 'upload', tag, str(path), '--repo', repo], check=True)
        current = checked_draft()
        if {a['name'] for a in current['assets']} != set(expected):
            raise SystemExit('Draft asset set mismatch')
        subprocess.run(['gh', 'release', 'download', tag, '--repo', repo,
                        '--dir', str(root / 'downloaded')], check=True)
        subprocess.run([sys.executable, verifier, '--directory', str(root / 'downloaded'),
                        '--tag', tag, '--commit', commit], check=True)
        for name, digest in expected.items():
            if hashlib.sha256((root / 'downloaded' / name).read_bytes()).hexdigest() != digest:
                raise SystemExit('Uploaded/downloaded bytes differ')
        checked_draft()
        Path('draft-verification.json').write_text(json.dumps({
            'draft': True, 'published': False, 'commit': commit, 'tag': tag,
            'checked_run': run_id, 'url': current['html_url'], 'sha256': expected,
        }, indent=2) + '\n')
        print('Verified draft assets; release remains unpublished:', current['html_url'])


if __name__ == '__main__':
    main()
