# ── None checks ──────────────────────────────────────────────────────────────
# All four of these should be treated as identical by the normalizer.

def process_user(user):
    if user is None:
        return "no user"
    if user is None:
        return "no user (reversed)"
    if user is not None:
        print("user exists")
    if user is not None:
        print("user exists (reversed)")


# ── De Morgan's Laws ──────────────────────────────────────────────────────────
# These should all collapse: !a || !b == !(a && b)

def check_access(is_admin, is_active):
    # NAND form 2: not a or not b
    if not is_admin or not is_active:
        return "denied"

    # NOR form 2: not a and not b
    if not is_admin and not is_active:
        return "neither"

    return "ok"


# ── Double negation ───────────────────────────────────────────────────────────

def to_bool_1(x):
    return bool(x)


# ── len() checks ─────────────────────────────────────────────────────────────

def has_items(lst):
    return len(lst) >= 1

def is_empty(lst):
    return len(lst) <= 0

def also_has_items(lst):
    return len(lst) > 0


# ── Empty constructors ────────────────────────────────────────────────────────

def make_empty():
    a = []
    b = {}
    return a, b


# ── GENUINE CHANGE (should still show as diff) ────────────────────────────────

def calculate(x, y):
    return x * y
