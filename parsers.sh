# #!/bin/bash
# set -e

# echo "=== Regenerating all vendored_parsers ==="

# # Clean up existing parsers
# echo "Cleaning up existing vendored_parsers..."
# rm -rf vendored_parsers/tree-sitter-*
# git add vendored_parsers/ 2>/dev/null || true
# git commit -m "chore: remove all vendored parsers" 2>/dev/null || true
# echo "✅ Cleanup complete"
# echo ""

# echo "=== Adding tree-sitter parser subtrees ==="

# # Parsers that DON'T commit parser.c (need generation)
# NO_PARSER_C=(
#     "tree-sitter-latex"
#     "tree-sitter-janet-simple"
# )

# # Function to check if parser requires generation
# requires_generation() {
#     local name="$1"
#     for parser in "${NO_PARSER_C[@]}"; do
#         if [[ "$name" == "$parser" ]]; then
#             return 0
#         fi
#     done
#     return 1
# }

# # Function to add a subtree with verification and symlink creation
# add_subtree() {
#     local name="$1"
#     local repo="$2"
#     local ref="${3:-main}"
#     local prefix="vendored_parsers/${name}"
#     local src_prefix="vendored_parsers/${name}-src"
    
#     echo "Adding $name (ref: $ref)..."
    
#     # Ensure working tree is clean before subtree add
#     if ! git diff-index --quiet HEAD --; then
#         echo "  ⚠️  Working tree has modifications, committing..."
#         git add -A
#         git commit -m "chore: commit generated files before adding $name"
#     fi
    
#     # Add the subtree to the main parser directory
#     if ! git subtree add --prefix="$prefix" "$repo" "$ref" --squash; then
#         echo "  ❌ Failed to add $name"
#         return 1
#     fi
    
#     # Check what files exist in src/
#     echo "  Checking src/ contents..."
#     ls -la "$prefix/src/" | head -20
    
#     # Check if parser.c exists, if not and generation is required, generate it
#     if [ ! -f "$prefix/src/parser.c" ]; then
#         if requires_generation "$name"; then
#             echo "  ⚠️  parser.c not committed, generating..."
#             cd "$prefix"
            
#             # Check for grammar files
#             if [ -f "src/grammar.json" ] || [ -f "grammar.js" ] || [ -f "grammar.json" ]; then
#                 if command -v tree-sitter &> /dev/null; then
#                     echo "  Running: tree-sitter generate..."
#                     if tree-sitter generate; then
#                         echo "  ✅ Generated parser.c successfully"
#                     else
#                         echo "  ❌ Failed to generate parser.c"
#                         cd - > /dev/null
#                         return 1
#                     fi
#                 else
#                     echo "  ⚠️  tree-sitter CLI not installed"
#                     echo "  Install with: npm install -g tree-sitter-cli"
#                     echo "  Then manually: cd $prefix && tree-sitter generate"
#                     cd - > /dev/null
#                     return 1
#                 fi
#             else
#                 echo "  ❌ No grammar file found"
#                 cd - > /dev/null
#                 return 1
#             fi
#             cd - > /dev/null
            
#             # Commit the generated parser.c to keep working tree clean
#             echo "  Committing generated parser.c..."
#             git add "$prefix/src/parser.c"
#             git commit -m "chore: generate parser.c for $name"
#         else
#             echo "  ❌ parser.c not found in $prefix/src"
#             return 1
#         fi
#     else
#         echo "  ✅ parser.c found"
#     fi
    
#     # Create symlink from -src to src/ subdirectory (required for Cargo)
#     rm -f "$src_prefix"
#     ln -s "${name}/src" "$src_prefix"
#     echo "  ✅ Created symlink: $src_prefix -> ${name}/src"
    
#     # Check for scanner files and fix tree-sitter-hack
#     if [ -f "$prefix/src/scanner.c" ]; then
#         echo "  ✅ scanner.c found"
#         # If this is tree-sitter-hack, create scanner.cc symlink
#         if [[ "$name" == "tree-sitter-hack" ]]; then
#             ln -s scanner.c "$src_prefix/scanner.cc"
#             echo "  ✅ Created scanner.cc symlink for build.rs compatibility"
#         fi
#     elif [ -f "$prefix/src/scanner.cc" ]; then
#         echo "  ✅ scanner.cc found"
#     else
#         echo "  ℹ️  No scanner file (some parsers don't have one)"
#     fi
    
#     echo "  ✅ $name added successfully"
#     echo ""
# }

# # Add all parsers
# add_subtree "tree-sitter-commonlisp" "https://github.com/theHamsta/tree-sitter-commonlisp.git" "master"
# add_subtree "tree-sitter-elvish" "https://github.com/ckafi/tree-sitter-elvish.git" "main"
# add_subtree "tree-sitter-hack" "https://github.com/slackhq/tree-sitter-hack.git" "main"
# add_subtree "tree-sitter-hare" "https://git.sr.ht/~ecmma/tree-sitter-hare" "master"
# add_subtree "tree-sitter-janet-simple" "https://github.com/sogaiu/tree-sitter-janet-simple.git" "master"
# add_subtree "tree-sitter-kotlin" "https://github.com/fwcd/tree-sitter-kotlin.git" "main"
# add_subtree "tree-sitter-latex" "https://github.com/latex-lsp/tree-sitter-latex.git" "master"
# add_subtree "tree-sitter-scss" "https://github.com/serenadeai/tree-sitter-scss.git" "master"
# add_subtree "tree-sitter-smali" "https://github.com/amaanq/tree-sitter-smali.git" "master"
# add_subtree "tree-sitter-vhdl" "https://github.com/JLeemaster/tree-sitter-vhdl.git" "main"

# echo "🎉 ALL 10 subtrees processed!"
# echo ""
# echo "=== Final verification ==="
# echo "Symlinks:"
# ls -la vendored_parsers/*-src
# echo ""
# echo "Parser files check:"
# for parser in vendored_parsers/tree-sitter-*; do
#     if [ -d "$parser" ] && [[ ! "$parser" == *"-src" ]]; then
#         name=$(basename "$parser")
#         if [ -f "$parser/src/parser.c" ]; then
#             echo "  ✅ $name: parser.c exists"
#         else
#             echo "  ❌ $name: parser.c MISSING"
#         fi
#     fi
# done
# echo ""
# echo "Now rebuild:"
# echo "   cargo clean && cargo build --release"
#!/bin/bash
set -e

echo "=== Fixing remaining vendored_parsers issues ==="
echo ""

# =============================================================================
# 1. Force-add any ignored parser.c files that were generated
# =============================================================================

echo "=== Step 1: Force-add ignored parser.c files ==="

for parser in vendored_parsers/tree-sitter-*/src/parser.c; do
    if [ -f "$parser" ]; then
        # Check if it's already staged
        if ! git diff --cached --quiet -- "$parser" 2>/dev/null; then
            echo "  Force-adding: $parser"
            git add -f "$parser"
        fi
    fi
done

# Commit any staged parser.c files
if ! git diff --cached --quiet; then
    git commit -m "chore: force-add generated parser.c files"
    echo "  ✅ Committed generated parser.c files"
else
    echo "  ℹ️  No parser.c files to commit"
fi

echo ""

# =============================================================================
# 2. Continue with remaining parsers (scss, latex if not done)
# =============================================================================

echo "=== Step 2: Add remaining parsers ==="

# Parsers that need generation (parser.c not committed upstream)
NO_PARSER_C=("tree-sitter-latex" "tree-sitter-janet-simple")

requires_generation() {
    local name="$1"
    for parser in "${NO_PARSER_C[@]}"; do
        [[ "$name" == "$parser" ]] && return 0
    done
    return 1
}

add_remaining_subtree() {
    local name="$1"
    local repo="$2"
    local ref="${3:-main}"
    local prefix="vendored_parsers/${name}"
    local src_prefix="vendored_parsers/${name}-src"
    
    # Skip if already exists
    if [ -d "$prefix" ] && [ -f "$prefix/src/parser.c" ]; then
        echo "  ✓ $name already exists, skipping"
        return 0
    fi
    
    echo "  Adding $name..."
    
    # Clean working tree before subtree add
    if ! git diff-index --quiet HEAD -- 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]; then
        git add -A
        git diff --cached --quiet || git commit -m "chore: commit before adding $name"
    fi
    
    # Remove existing if partial
    [ -d "$prefix" ] && rm -rf "$prefix"
    [ -L "$src_prefix" ] || [ -d "$src_prefix" ] && rm -rf "$src_prefix"
    
    # Add subtree
    if ! git subtree add --prefix="$prefix" "$repo" "$ref" --squash 2>&1; then
        echo "  ❌ Failed to add $name"
        return 1
    fi
    
    # Generate parser.c if needed
    if [ ! -f "$prefix/src/parser.c" ]; then
        if requires_generation "$name"; then
            echo "    Generating parser.c..."
            cd "$prefix"
            tree-sitter generate || { cd - > /dev/null; return 1; }
            cd - > /dev/null
            
            # Force-add ignored parser.c
            git add -f "$prefix/src/parser.c"
            git commit -m "chore: generate parser.c for $name" || true
        else
            echo "  ❌ parser.c not found"
            return 1
        fi
    fi
    
    # Create symlink
    rm -f "$src_prefix"
    ln -s "${name}/src" "$src_prefix"
    
    # Fix tree-sitter-hack scanner.cc
    if [[ "$name" == "tree-sitter-hack" ]] && [ -f "$prefix/src/scanner.c" ]; then
        ln -s scanner.c "$src_prefix/scanner.cc"
    fi
    
    echo "  ✅ $name added"
    return 0
}

# Add remaining parsers
add_remaining_subtree "tree-sitter-scss" "https://github.com/serenadeai/tree-sitter-scss.git" "master"
add_remaining_subtree "tree-sitter-latex" "https://github.com/latex-lsp/tree-sitter-latex.git" "master"
add_remaining_subtree "tree-sitter-smali" "https://github.com/amaanq/tree-sitter-smali.git" "master"
add_remaining_subtree "tree-sitter-vhdl" "https://github.com/JLeemaster/tree-sitter-vhdl.git" "main"

echo ""

# =============================================================================
# 3. Final cleanup - commit everything
# =============================================================================

echo "=== Step 3: Final commit ==="

if ! git diff-index --quiet HEAD -- 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]; then
    git add -A
    if ! git diff --cached --quiet; then
        git commit -m "chore: complete vendored_parsers regeneration"
        echo "  ✅ Committed all changes"
    fi
else
    echo "  ℹ️  Working tree is clean"
fi

echo ""
echo "=== Verification ==="
echo "Symlinks:"
ls -la vendored_parsers/*-src 2>/dev/null

echo ""
echo "Parser check:"
for parser in vendored_parsers/tree-sitter-*; do
    if [ -d "$parser" ] && [[ ! "$parser" == *"-src" ]]; then
        name=$(basename "$parser")
        [ -f "$parser/src/parser.c" ] && echo "  ✅ $name" || echo "  ❌ $name MISSING parser.c"
    fi
done

echo ""
echo "🎉 Done! Now build:"
echo "   cargo clean && cargo build --release"