#!/usr/bin/env python3
"""Real-git positive/negative regression for Agent C scoped-PR policy."""
from __future__ import annotations
import subprocess
import tempfile
import unittest
from pathlib import Path
from check_scoped_quantum_pr import verify,PolicyError,V3

C="quantum/agent-c-q28-calibration-parity-004"
def run(p,*args):
    return subprocess.run(["git","-C",str(p),*args],capture_output=True,text=True,check=True).stdout.strip()

class ScopeTests(unittest.TestCase):
    def setUp(self):
        d=tempfile.TemporaryDirectory()
        self.addCleanup(d.cleanup)
        self.root=Path(d.name)
        run(self.root,"init","-q")
        run(self.root,"config","user.email","test@example.invalid")
        run(self.root,"config","user.name","CI")
        self.write("README.md","base")
        self.commit()
        self.base=run(self.root,"rev-parse","HEAD")
    def write(self,path,data):
        x=self.root/path
        x.parent.mkdir(parents=True,exist_ok=True)
        x.write_text(data)
    def commit(self):
        run(self.root,"add","-A")
        run(self.root,"commit","-qm","test")
        return run(self.root,"rev-parse","HEAD")
    def feature(self,source=True,note="Agent C | Issue #107 | Draft PR #108",extras=()):
        if source:self.write("aura-quantum-sim/src/lib.rs","pub fn run() {}\n")
        if note is not None:self.write("docs/quantum/agent-c/C-012.md",note)
        for path in extras:self.write(path,"changed")
        return self.commit()
    def verdict(self,sha,pr=108,head=C,base=V3):
        return verify(self.root,self.base,sha,pr,head,base)
    def test_valid(self):self.assertIn("SCOPE_PASS",self.verdict(self.feature()))
    def test_scope_workflow(self):
        self.assertIn("SCOPE_PASS",self.verdict(self.feature(extras=(".github/workflows/quantum-reference-sim.yml",))))
    def test_steward_policy_separate(self):
        self.write("tools/qualification/check_governance.py","# new")
        self.assertIn("NOT_APPLICABLE",self.verdict(self.commit(),head="integration/x003-v3-scoped-governance"))
    def test_no_source(self):
        with self.assertRaisesRegex(PolicyError,"simulator"):self.verdict(self.feature(source=False))
    def test_no_note(self):
        with self.assertRaisesRegex(PolicyError,"evidence"):self.verdict(self.feature(note=None))
    def test_no_issue(self):
        with self.assertRaisesRegex(PolicyError,"Issue claim"):self.verdict(self.feature(note="Draft PR #108"))
    def test_wrong_pr(self):
        with self.assertRaisesRegex(PolicyError,"Draft PR"):self.verdict(self.feature(note="Issue #107 Draft PR #999"))
    def test_not_draft(self):
        with self.assertRaisesRegex(PolicyError,"Draft PR"):self.verdict(self.feature(note="Issue #107 PR #108"))
    def test_no_foreign_core(self):
        with self.assertRaisesRegex(PolicyError,"foreign"):self.verdict(self.feature(extras=("aura-core/src/lower.rs",)))
    def test_no_foreign_lumina(self):
        with self.assertRaisesRegex(PolicyError,"foreign"):self.verdict(self.feature(extras=("aura-plugin-lumina/src/lib.rs",)))
    def test_no_shared_root(self):
        with self.assertRaisesRegex(PolicyError,"foreign"):self.verdict(self.feature(extras=("Cargo.toml",)))
    def test_no_roadmap_edit(self):
        with self.assertRaisesRegex(PolicyError,"foreign"):self.verdict(self.feature(extras=("docs/qualification/MASTER_ROADMAP_V3.md",)))
    def test_no_shared_ledgers_even_paired(self):
        with self.assertRaisesRegex(PolicyError,"foreign"):self.verdict(self.feature(extras=("docs/qualification/PROGRESS.md","docs/qualification/CONTEXT_HANDOFF.md")))
    def test_no_global_workflow(self):
        with self.assertRaisesRegex(PolicyError,"foreign"):self.verdict(self.feature(extras=(".github/workflows/ci.yml",)))
    def test_no_unknown_quantum_ref(self):
        with self.assertRaisesRegex(PolicyError,"unclaimed"):self.verdict(self.feature(),head="quantum/unknown")
    def test_wrong_base(self):
        with self.assertRaisesRegex(PolicyError,"base"):self.verdict(self.feature(),base="main")
    def test_symlink_denied(self):
        self.write("aura-quantum-sim/src/lib.rs","pub fn run() {}")
        self.write("docs/quantum/agent-c/C-012.md","Issue #107 Draft PR #108")
        (self.root/"aura-quantum-sim/src/link.rs").symlink_to("lib.rs")
        with self.assertRaisesRegex(PolicyError,"symlink"):self.verdict(self.commit())
    def test_empty_diff(self):
        with self.assertRaisesRegex(PolicyError,"empty"):self.verdict(self.base)

if __name__=="__main__":
    result=unittest.TextTestRunner(verbosity=1).run(unittest.defaultTestLoader.loadTestsFromTestCase(ScopeTests))
    if not result.wasSuccessful():raise SystemExit(1)
    print("X003_SCOPED_GOVERNANCE_TESTS_PASS cases="+str(result.testsRun))
