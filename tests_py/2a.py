# ── None checks ──────────────────────────────────────────────────────────────
# All four of these should be treated as identical by the normalizer.

def process_user(user):
    if user == None:
        return "no user"
    if None == user:
        return "no user (reversed)"
    if user != None:
        print("user exists")
    if None != user:
        print("user exists (reversed)")


# ── De Morgan's Laws ──────────────────────────────────────────────────────────
# These should all collapse: !(a && b) == !a || !b

def check_access(is_admin, is_active):
    # NAND form 1: not (a and b)
    if not (is_admin and is_active):
        return "denied"

    # NOR form 1: not (a or b)
    if not (is_admin or is_active):
        return "neither"

    return "ok"


# ── Double negation ───────────────────────────────────────────────────────────

def to_bool_1(x):
    return not not x


# ── len() checks ─────────────────────────────────────────────────────────────

def has_items(lst):
    return len(lst) > 0

def is_empty(lst):
    return len(lst) == 0

def also_has_items(lst):
    return len(lst) != 0


# ── Empty constructors ────────────────────────────────────────────────────────

def make_empty():
    a = list()
    b = dict()
    return a, b


# ── GENUINE CHANGE (should still show as diff) ────────────────────────────────

def calculate(x, y):
    return x + y
