#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}✓${NC} $1"; }
log_warn() { echo -e "${YELLOW}⚠${NC} $1"; }
log_error() { echo -e "${RED}✗${NC} $1"; }

echo "=== Regenerating all vendored_parsers ==="
echo ""

# =============================================================================
# PREREQUISITE CHECKS
# =============================================================================

echo "=== Prerequisite Checks ==="

# Check if tree-sitter CLI is installed
if ! command -v tree-sitter &> /dev/null; then
    log_error "tree-sitter CLI not found!"
    echo "  Install with: npm install -g tree-sitter-cli"
    echo "  Or: cargo install tree-sitter-cli"
    exit 1
fi
log_info "tree-sitter CLI found: $(tree-sitter --version)"

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    log_error "Not in a git repository!"
    exit 1
fi
log_info "Git repository detected"

echo ""

# =============================================================================
# CLEANUP
# =============================================================================

echo "=== Cleaning up existing parsers ==="

# Remove existing parser directories
if [ -d "vendored_parsers" ]; then
    find vendored_parsers -name "tree-sitter-*" -type d -exec chmod -R u+w {} \; 2>/dev/null || true
    rm -rf vendored_parsers/tree-sitter-*
    log_info "Removed existing tree-sitter-* directories"
else
    mkdir -p vendored_parsers
    log_info "Created vendored_parsers directory"
fi

# Clean working tree
if ! git diff-index --quiet HEAD -- 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]; then
    log_warn "Working tree has modifications"
    git add -A
    if git diff --cached --quiet; then
        log_info "No staged changes to commit"
    else
        git commit -m "chore: save work before regenerating vendored parsers"
        log_info "Committed pending changes"
    fi
else
    log_info "Working tree is clean"
fi

echo ""

# =============================================================================
# CONFIGURATION
# =============================================================================

# Parsers that DON'T commit parser.c (need generation)
NO_PARSER_C=("tree-sitter-latex" "tree-sitter-janet-simple")

# Parser configurations
declare -a PARSERS=(
    "tree-sitter-commonlisp:https://github.com/theHamsta/tree-sitter-commonlisp.git:master"
    "tree-sitter-elvish:https://github.com/ckafi/tree-sitter-elvish.git:main"
    "tree-sitter-hack:https://github.com/slackhq/tree-sitter-hack.git:main"
    "tree-sitter-hare:https://git.sr.ht/~ecmma/tree-sitter-hare:master"
    "tree-sitter-janet-simple:https://github.com/sogaiu/tree-sitter-janet-simple.git:master"
    "tree-sitter-kotlin:https://github.com/fwcd/tree-sitter-kotlin.git:main"
    "tree-sitter-latex:https://github.com/latex-lsp/tree-sitter-latex.git:master"
    "tree-sitter-scss:https://github.com/serenadeai/tree-sitter-scss.git:master"
    "tree-sitter-smali:https://github.com/amaanq/tree-sitter-smali.git:master"
    "tree-sitter-vhdl:https://github.com/JLeemaster/tree-sitter-vhdl.git:main"
)

# =============================================================================
# HELPER FUNCTIONS
# =============================================================================

requires_generation() {
    local name="$1"
    for parser in "${NO_PARSER_C[@]}"; do
        [[ "$name" == "$parser" ]] && return 0
    done
    return 1
}

commit_changes_if_any() {
    local message="$1"
    local force_add="$2"  # Pass "-f" to force add ignored files
    
    if ! git diff --cached --quiet; then
        if [ "$force_add" = "-f" ]; then
            git commit -m "$message"
        else
            git commit -m "$message"
        fi
        return 0
    fi
    return 1
}

# =============================================================================
# MAIN FUNCTION
# =============================================================================

add_subtree() {
    local name="$1"
    local repo="$2"
    local ref="${3:-main}"
    local prefix="vendored_parsers/${name}"
    local src_prefix="vendored_parsers/${name}-src"
    
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Adding $name (ref: $ref)"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Ensure working tree is clean BEFORE subtree add
    if ! git diff-index --quiet HEAD -- 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]; then
        log_warn "Working tree has modifications, committing..."
        git add -A
        if ! commit_changes_if_any "chore: commit before adding $name"; then
            log_info "No changes to commit"
        fi
    fi
    
    # Remove existing subtree directory if it exists
    if [ -d "$prefix" ]; then
        log_warn "Directory $prefix already exists, removing..."
        rm -rf "$prefix"
        git add "$prefix" 2>/dev/null || true
        commit_changes_if_any "chore: remove existing $name before re-adding" || true
    fi
    
    # Remove existing symlink if it exists
    if [ -L "$src_prefix" ] || [ -d "$src_prefix" ]; then
        rm -rf "$src_prefix"
    fi
    
    # Add the subtree
    echo "  Fetching from $repo ($ref)..."
    if ! git subtree add --prefix="$prefix" "$repo" "$ref" --squash 2>&1; then
        log_error "Failed to add $name"
        return 1
    fi
    log_info "Subtree added successfully"
    
    # Verify src/ directory exists
    if [ ! -d "$prefix/src" ]; then
        log_error "src/ directory not found in $prefix"
        return 1
    fi
    log_info "src/ directory found"
    
    # Show src/ contents
    echo "  src/ contents:"
    ls -la "$prefix/src/" | head -15
    
    # Check if parser.c exists, generate if needed
    if [ ! -f "$prefix/src/parser.c" ]; then
        if requires_generation "$name"; then
            log_warn "parser.c not committed, generating..."
            cd "$prefix"
            
            # Find grammar file
            local grammar_location=""
            if [ -f "src/grammar.json" ]; then
                grammar_location="src/grammar.json"
            elif [ -f "grammar.js" ]; then
                grammar_location="grammar.js"
            elif [ -f "grammar.json" ]; then
                grammar_location="grammar.json"
            elif [ -f "src/grammar.js" ]; then
                grammar_location="src/grammar.js"
            fi
            
            if [ -z "$grammar_location" ]; then
                log_error "No grammar file found"
                cd - > /dev/null
                return 1
            fi
            
            log_info "Found grammar at: $grammar_location"
            echo "  Running: tree-sitter generate..."
            
            if ! tree-sitter generate 2>&1; then
                log_error "tree-sitter generate failed"
                cd - > /dev/null
                return 1
            fi
            
            log_info "Generated parser.c successfully"
            cd - > /dev/null
            
            # KEY FIX: Force add ignored files with -f flag
            log_warn "parser.c may be ignored by .gitignore, force adding..."
            git add -f "$prefix/src/parser.c"
            log_info "Force-added parser.c (ignored by .gitignore)"
            
            # Commit the generated file
            if git diff --cached --quiet; then
                log_info "No changes to commit"
            else
                git commit -m "chore: generate parser.c for $name"
                log_info "Committed generated parser.c"
            fi
        else
            log_error "parser.c not found and $name doesn't require generation"
            return 1
        fi
    else
        log_info "parser.c found"
    fi
    
    # Verify parser.c exists after generation
    if [ ! -f "$prefix/src/parser.c" ]; then
        log_error "parser.c still not found after generation attempt"
        return 1
    fi
    
    # Create symlink from -src to src/
    rm -f "$src_prefix"
    ln -s "${name}/src" "$src_prefix"
    if [ -L "$src_prefix" ]; then
        log_info "Created symlink: $src_prefix -> ${name}/src"
    else
        log_error "Failed to create symlink"
        return 1
    fi
    
    # Handle scanner files
    local has_scanner=false
    if [ -f "$prefix/src/scanner.c" ]; then
        log_info "scanner.c found"
        has_scanner=true
        # For tree-sitter-hack, create scanner.cc symlink in -src dir
        if [[ "$name" == "tree-sitter-hack" ]]; then
            ln -s scanner.c "$src_prefix/scanner.cc"
            log_info "Created scanner.cc symlink for build.rs compatibility"
        fi
    elif [ -f "$prefix/src/scanner.cc" ]; then
        log_info "scanner.cc found"
        has_scanner=true
    fi
    
    if [ "$has_scanner" = false ]; then
        log_info "No scanner file (some parsers don't have one)"
    fi
    
    log_info "$name added successfully ✅"
    return 0
}

# =============================================================================
# MAIN EXECUTION
# =============================================================================

echo "=== Adding tree-sitter parser subtrees ==="

failed_parsers=()
successful_parsers=()

for parser_config in "${PARSERS[@]}"; do
    IFS=':' read -r name repo ref <<< "$parser_config"
    if add_subtree "$name" "$repo" "$ref"; then
        successful_parsers+=("$name")
    else
        failed_parsers+=("$name")
        log_error "Failed to add $name - continuing with next parser..."
    fi
done

# =============================================================================
# FINAL SUMMARY
# =============================================================================

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "=== FINAL SUMMARY ==="
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo ""
echo "Successful (${#successful_parsers[@]}):"
for p in "${successful_parsers[@]}"; do
    echo "  ✅ $p"
done

if [ ${#failed_parsers[@]} -gt 0 ]; then
    echo ""
    echo "Failed (${#failed_parsers[@]}):"
    for p in "${failed_parsers[@]}"; do
        echo "  ❌ $p"
    done
    echo ""
    log_warn "Some parsers failed. Fix issues and re-run script."
    exit 1
fi

echo ""
echo "=== Verification ==="
echo ""
echo "Symlinks:"
ls -la vendored_parsers/*-src 2>/dev/null || echo "  No symlinks found"

echo ""
echo "Parser files:"
for parser in vendored_parsers/tree-sitter-*; do
    if [ -d "$parser" ] && [[ ! "$parser" == *"-src" ]]; then
        name=$(basename "$parser")
        if [ -f "$parser/src/parser.c" ]; then
            echo "  ✅ $name: parser.c"
        else
            echo "  ❌ $name: parser.c MISSING"
        fi
    fi
done

echo ""
echo "=== tree-sitter-hack scanner check ==="
if [ -L "vendored_parsers/tree-sitter-hack-src/scanner.cc" ]; then
    echo "  ✅ scanner.cc symlink exists"
    ls -la vendored_parsers/tree-sitter-hack-src/scanner.*
else
    echo "  ⚠️  scanner.cc symlink not found"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
log_info "ALL PARSERS ADDED SUCCESSFULLY! 🎉"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Next steps:"
echo "  1. Review changes: git status"
echo "  2. Build: cargo clean && cargo build --release"
echo ""