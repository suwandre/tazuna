#!/usr/bin/env python3
"""The providers provgauge.sh checks: every one SubRouter lists for MODEL at a listed input price above 0 and up to
$0.03/M (Cybersol lists $0 and bills ~4x LingAI, 2026-11-01; the $0.09+ ones cost 4-8x for the same model)."""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import subrouter as s  # noqa: E402

m = {x["model_name"]: x for x in s.api("/api/user/self/subrouter/models")}[sys.argv[1]]
print(" ".join(p["provider_slug"] for p in m["providers"] if 0 < p["min_input_price"] <= 0.03))
