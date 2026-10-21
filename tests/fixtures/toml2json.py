#!/usr/bin/env python3
# For jq-based bats assertions instead of grep/line-offset matching.
import json
import sys
import tomllib

with open(sys.argv[1], "rb") as f:
    json.dump(tomllib.load(f), sys.stdout)
