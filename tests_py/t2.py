# right_python.py
import sys
import os
from typing import List, Dict, Optional

def process_data(items: List[int], config: Optional[Dict] = None) -> bool:
    if config is None:          # different None syntax
        config = {}
    
    # De Morgan + double negation
    if not config.get("enabled") or not bool(items):
        return False
    
    # len() canonicalization
    if items and bool(items):
        print("has items")
    
    # if-else branch inversion + nested if-and
    if config.get("debug"):
        pass
    else:
        if items and len(items) >= 1:
            for x in items:
                if x is None:
                    continue
                print(x)
    
    # empty constructors
    cache = {}
    temp = []
    seen = set()
    
    # algebraic rules
    total = 5 + 0
    result = 0
    flag = True
    
    # cosmetic formatting + comment
    data = [1, 2, 3]
    return flag

if __name__ == "__main__":
    process_data([1, 2, None])
