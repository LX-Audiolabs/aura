"""Check real rustc x86_64/FMA assembly; no source-text matching.

Build: cargo rustc -p lx-aura-dsp --example block_codegen --release -- --emit=asm -C target-cpu=native
Run: python crates/aura-dsp/tools/check_codegen.py target/release/examples/block_codegen.s

Needs an FMA-capable x86_64 target. This is an architecture-specific regression
check, not a claim about all LLVM versions/targets or IIR vectorization over time.
"""
import re
import sys
from pathlib import Path


def body(assembly, name):
    match = re.search(
        r"^" + name + r":\n(.*?)(?=^\s*\.seh_endproc)",
        assembly,
        re.MULTILINE | re.DOTALL,
    )
    if not match:
        raise AssertionError(f"missing Windows x86_64 assembly symbol: {name}")
    return match[1]


def violations(code):
    return [label for label, pattern in (
        ("double audio arithmetic", r"\b(?:v?mul|v?add|v?sub|v?div)sd\b|\bvf\w+sd\b"),
        ("gather", r"\bv?gather\w*\b"),
        ("FIR history mask", r"\band[lq]?\s+\$(?:63|127),"),
        ("integer modulo/division", r"\bi?div[lq]\b"),
    ) if re.search(pattern, code)]


def main():
    assembly = Path(sys.argv[1]).read_text(encoding="utf-8")
    legacy = violations(body(assembly, "probe_legacy_fir"))
    assert "double audio arithmetic" in legacy, "legacy negative control no longer detects f64"
    assert "FIR history mask" in legacy, "legacy negative control no longer detects wrapped taps"
    print("BASELINE FAIL (expected): legacy FIR:", ", ".join(legacy))
    for name in ("probe_mix", "probe_iir", "probe_dc", "probe_delay"):
        code = body(assembly, name)
        errors = violations(code)
        if name == "probe_delay":
            # Delay wrap/modulo is allowed BETWEEN linear interpolation spans.
            errors = [e for e in errors if e != "integer modulo/division"]
            spans = [
                b
                for b in re.split(r"^\.LBB[^\n]*:\n", code, flags=re.MULTILINE)
                if "vmulps" in b
            ]
            assert spans, "missing vector delay span"
            assert all("integer modulo/division" not in violations(b) for b in spans), "delay interpolation wraps inside a vector span"
        assert not errors, f"{name}: {errors}"
        if name in ("probe_mix", "probe_iir"):
            assert re.search(r"\bvfmadd\d+ps\b", code), f"{name}: missing packed f32 FMA"
        elif name == "probe_delay":
            assert "vmulps" in code, "delay interpolation is no longer vectorized"
        else:
            assert re.search(r"\bvfmadd\d+ss\b", code), "DC recurrence missing scalar f32 FMA"
        print("PASS:", name, "f32, no gather or wrapped history MAC")
    # Biquad preparation may legitimately contain f64 outside its f32 tick.
    assert re.search(r"\bvfmadd\d+ss\b", body(assembly, "probe_biquad"))
    print("PASS: probe_biquad scalar f32 FMA (f64 coefficient preparation allowed)")


if __name__ == "__main__":
    main()
