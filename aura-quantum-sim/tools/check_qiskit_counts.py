#!/usr/bin/env python3
"""C-015 real offline 65k shot counts vs Qiskit Born probabilities; no QPU."""
import json,math,platform,subprocess,sys
from importlib.metadata import version
from pathlib import Path
from qiskit import QuantumCircuit
from qiskit.quantum_info import Statevector
PKGS={"qiskit":"2.6.1","numpy":"2.3.4"}
def run(binary,path):
    raw=path.read_bytes()
    p=subprocess.run([str(binary.resolve())],input=raw,capture_output=True,timeout=45)
    if p.returncode or p.stderr:
        raise AssertionError("counts CLI failure "+str(p.stderr[:500]))
    d=json.loads(p.stdout)
    if (d["schema"]!="aura.quantum.factorized-counts-result/1"
        or d["retained_shot_records"]!=0 or d["counts_only"] is not True
        or d["qpu_hardware_jobs"]!=0 or "results" in d or "trace" in d
        or len(d["histogram"])>4096 or sum(d["histogram"].values())!=d["shots"]
        or len(p.stdout)>4096):
        raise AssertionError("wrong counts-only/histogram/provenance contract")
    return p.stdout,d
def main(binary,fixtures,output):
    versions={k:version(k) for k in PKGS}
    if versions!=PKGS or sys.version_info[:2]!=(3,11):
        raise RuntimeError("not Q28 pinned Qiskit environment "+str(versions))
    one=QuantumCircuit(1);one.h(0)
    two=QuantumCircuit(2);two.h(0);two.cx(0,1)
    cases=[("hadamard",65536,one),("bell",32768,two)]
    result={"status":"Q28_HIGH_SHOT_BORN_RULE_PASS","shots":0,"cases":[],
            "max_z":0.0,"negative_zero_probability_controls":0,
            "seed_replay":"byte-identical","trace_records_retained":0,
            "qpu_hardware_jobs":0,"packages":versions,
            "python":platform.python_version(),
            "confidence":"6_sigma_binomial_not_formal_proof"}
    for name,n,circuit in cases:
        path=fixtures/("factorized_counts_"+name+"_v1.json")
        raw,data=run(binary,path)
        second,_=run(binary,path)
        if raw!=second or data["shots"]!=n:
            raise AssertionError("seed replay / shot count mismatch")
        probs=Statevector.from_instruction(circuit).probabilities()
        if len(probs)!=(1<<circuit.num_qubits):raise AssertionError("reference width")
        for i,p in enumerate(probs):
            bitstring=format(i,f"0{circuit.num_qubits}b")
            observed=data["histogram"].get(bitstring,0)
            if p==0:
                if observed!=0:raise AssertionError("impossible outcome observed")
                result["negative_zero_probability_controls"]+=1
            else:
                sigma=math.sqrt(n*float(p)*(1-float(p)))
                deviation=abs(observed-n*float(p))
                if sigma==0:
                    if deviation!=0:raise AssertionError("deterministic outcome wrong")
                elif deviation>6*sigma:
                    raise AssertionError(f"Born deviation too large: {name}/{bitstring}")
                if sigma:result["max_z"]=max(result["max_z"],deviation/sigma)
        result["shots"]+=n
        result["cases"].append({"name":name,"shots":n,"bins":len(data["histogram"]),
                                "histogram":data["histogram"]})
    if result["shots"]!=98304 or len(result["cases"])!=2:
        raise AssertionError("incomplete counts proof corpus")
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(result,sort_keys=True,indent=2)+"\n")
    print("Q28_COUNTS_BORN_RULE "+json.dumps(result,sort_keys=True),flush=True)
if __name__=="__main__":
    if len(sys.argv)!=4:raise SystemExit("usage: check_qiskit_counts.py binary fixtures report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
