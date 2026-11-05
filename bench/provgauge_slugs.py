#!/usr/bin/env python3
"""The providers provgauge.sh checks: every one SubRouter lists for MODEL at a listed input price above 0 and at most
about twice the floor, the median of the three cheapest (one fake cheap provider, like fxpayback's "Sol" at a twelfth
of the others, cannot set it). Past that a subscription is cheaper than paying a provider's premium (user, 2026-11-05).
Cybersol lists $0 and bills ~4x LingAI (2026-11-01)."""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import subrouter as s  # noqa: E402

m = {x["model_name"]: x for x in s.api("/api/user/self/subrouter/models")}[sys.argv[1]]
priced = sorted((p for p in m["providers"] if p["min_input_price"] > 0), key=lambda p: p["min_input_price"])
floor = [p["min_input_price"] for p in priced[:3]][len(priced[:3]) // 2]
print(" ".join(p["provider_slug"] for p in priced if p["min_input_price"] <= 2.1 * floor))
