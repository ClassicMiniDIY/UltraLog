#!/bin/bash
#
# Version bump script for UltraLog
# Updates the version in all locations:
#   - Cargo.toml
#   - README.md (version badge)
#
# ultralog.co (ClassicMiniDIY/ultralog-site) takes its version from the GitHub release and
# rebuilds when release.yml publishes one, so there is no site file to bump here.
#
# Usage: ./scripts/bump-version.sh <new-version>
# Example: ./scripts/bump-version.sh 1.7.0
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if version argument is provided
if [ -z "$1" ]; then
    echo -e "${RED}Error: No version specified${NC}"
    echo ""
    echo "Usage: $0 <new-version>"
    echo "Example: $0 1.7.0"
    exit 1
fi

NEW_VERSION="$1"

# Validate version format (semver: X.Y.Z)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo -e "${RED}Error: Invalid version format '${NEW_VERSION}'${NC}"
    echo "Version must be in semver format: X.Y.Z (e.g., 1.7.0)"
    exit 1
fi

# Get the script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CARGO_TOML="$PROJECT_ROOT/Cargo.toml"
README="$PROJECT_ROOT/README.md"

# Check if Cargo.toml exists
if [ ! -f "$CARGO_TOML" ]; then
    echo -e "${RED}Error: Cargo.toml not found at $CARGO_TOML${NC}"
    exit 1
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')

if [ -z "$CURRENT_VERSION" ]; then
    echo -e "${RED}Error: Could not find current version in Cargo.toml${NC}"
    exit 1
fi

echo -e "${YELLOW}Current version:${NC} $CURRENT_VERSION"
echo -e "${GREEN}New version:${NC} $NEW_VERSION"
echo ""

# Check if versions are the same
if [ "$CURRENT_VERSION" = "$NEW_VERSION" ]; then
    echo -e "${YELLOW}Warning: New version is the same as current version${NC}"
    exit 0
fi

# Update Cargo.toml
echo "Updating Cargo.toml..."
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML"
rm -f "$CARGO_TOML.bak"

# Verify the Cargo.toml update
UPDATED_VERSION=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')
if [ "$UPDATED_VERSION" != "$NEW_VERSION" ]; then
    echo -e "${RED}Error: Cargo.toml version update failed${NC}"
    exit 1
fi
echo -e "  ${GREEN}Done${NC}"

# Update README.md version badge
if [ -f "$README" ]; then
    echo "Updating README.md..."
    sed -i.bak "s/version-$CURRENT_VERSION-/version-$NEW_VERSION-/g" "$README"
    rm -f "$README.bak"
    echo -e "  ${GREEN}Done${NC}"
else
    echo -e "  ${YELLOW}Skipped (file not found)${NC}"
fi

# Update Cargo.lock by running cargo check
echo "Updating Cargo.lock..."
cd "$PROJECT_ROOT"
cargo check --quiet 2>/dev/null || true
echo -e "  ${GREEN}Done${NC}"

echo ""
echo -e "${GREEN}Version bump complete!${NC}"
echo ""
echo "Files updated:"
echo "  - Cargo.toml"
echo "  - README.md"
echo "  - Cargo.lock"
echo ""
echo "Next steps:"
echo "  1. Review changes: git diff"
echo "  2. Commit changes: git commit -am \"Bump version to $NEW_VERSION\""
echo "  3. Create tag: git tag -a v$NEW_VERSION -m \"Release v$NEW_VERSION\""
echo "  4. Push changes: git push && git push --tags"
