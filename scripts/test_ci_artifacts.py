import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("artifacts", Path(__file__).with_name("ci-artifacts.py"))
artifacts = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifacts)

class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "evidence"
        self.source.mkdir()
        self.out = self.root / "out"
        for name in artifacts.NAMES:
            (self.source / name).write_text('{}' if name.endswith('.json') else 'passed\n')
    def stage(self, sha='a'*40, run='123'):
        return artifacts.stage(self.source,self.out,sha,run)
    def test_manifest_hashes_and_allowlist(self):
        (self.source / 'credentials.json').write_text('secret')
        manifest=self.stage()
        self.assertEqual(len(manifest['files']),5)
        self.assertFalse(manifest['gpu_validated'])
        for item in manifest['files']:
            import hashlib
            data=(self.out/item['path']).read_bytes()
            self.assertEqual(item['sha256'],hashlib.sha256(data).hexdigest())
            self.assertEqual(item['bytes'],len(data))
        self.assertFalse((self.out/'credentials.json').exists())
    def test_provenance_and_overwrite_rejected(self):
        for sha,run in [('main','123'),('a'*40,'../escape')]:
            with self.assertRaises(ValueError): self.stage(sha,run)
        self.stage()
        with self.assertRaises(ValueError): self.stage()
    def test_missing_empty_and_invalid_json_rejected(self):
        path=self.source/'benchmark.json'
        for data in ['', '{broken']:
            path.write_text(data)
            with self.assertRaises(ValueError): self.stage()
            self.assertFalse(self.out.exists())
        path.unlink()
        with self.assertRaises(ValueError): self.stage()
    def test_symlinks_rejected(self):
        path=self.source/'checks.txt'
        path.unlink()
        path.symlink_to(self.source/'benchmark.json')
        with self.assertRaises(ValueError): self.stage()

if __name__=='__main__': unittest.main()
