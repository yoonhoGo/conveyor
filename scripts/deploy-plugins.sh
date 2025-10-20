#!/bin/bash
set -e

VERSION="${1:-v0.11.0}"
REPO="yoonhoGo/conveyor"

echo "🚀 Manual Plugin Deployment for $VERSION"
echo "=========================================="

# Check if release exists
if ! gh release view "$VERSION" --repo "$REPO" > /dev/null 2>&1; then
    echo "❌ Release $VERSION not found. Creating release..."
    gh release create "$VERSION" \
        --repo "$REPO" \
        --title "Release $VERSION" \
        --notes "Plugin distribution system and improvements" \
        --draft
    echo "✅ Draft release created: $VERSION"
else
    echo "✅ Release $VERSION found"
fi

echo ""
echo "📦 Building plugins in release mode..."
echo "--------------------------------------"

# Build HTTP plugin
echo "Building HTTP plugin..."
cargo build --release -p conveyor-plugin-http
if [ $? -eq 0 ]; then
    echo "✅ HTTP plugin built"
else
    echo "❌ HTTP plugin build failed"
    exit 1
fi

# Build MongoDB plugin
echo "Building MongoDB plugin..."
cargo build --release -p conveyor-plugin-mongodb
if [ $? -eq 0 ]; then
    echo "✅ MongoDB plugin built"
else
    echo "❌ MongoDB plugin build failed"
    exit 1
fi

echo ""
echo "🔐 Generating checksums..."
echo "--------------------------------------"

cd target/release

# Generate checksums
HTTP_CHECKSUM=$(shasum -a 256 libconveyor_plugin_http.dylib | awk '{print $1}')
MONGO_CHECKSUM=$(shasum -a 256 libconveyor_plugin_mongodb.dylib | awk '{print $1}')

echo "HTTP Plugin SHA256:    $HTTP_CHECKSUM"
echo "MongoDB Plugin SHA256: $MONGO_CHECKSUM"

# Save checksums to files
echo "$HTTP_CHECKSUM  libconveyor_plugin_http.dylib" > libconveyor_plugin_http.dylib.sha256
echo "$MONGO_CHECKSUM  libconveyor_plugin_mongodb.dylib" > libconveyor_plugin_mongodb.dylib.sha256

cd ../..

echo ""
echo "📤 Uploading to GitHub Release..."
echo "--------------------------------------"

# Upload HTTP plugin
echo "Uploading HTTP plugin..."
gh release upload "$VERSION" \
    target/release/libconveyor_plugin_http.dylib \
    target/release/libconveyor_plugin_http.dylib.sha256 \
    --repo "$REPO" \
    --clobber

if [ $? -eq 0 ]; then
    echo "✅ HTTP plugin uploaded"
else
    echo "❌ HTTP plugin upload failed"
    exit 1
fi

# Upload MongoDB plugin
echo "Uploading MongoDB plugin..."
gh release upload "$VERSION" \
    target/release/libconveyor_plugin_mongodb.dylib \
    target/release/libconveyor_plugin_mongodb.dylib.sha256 \
    --repo "$REPO" \
    --clobber

if [ $? -eq 0 ]; then
    echo "✅ MongoDB plugin uploaded"
else
    echo "❌ MongoDB plugin upload failed"
    exit 1
fi

echo ""
echo "📝 Updating registry.json..."
echo "--------------------------------------"

# Update registry.json with checksums
cat > registry.json <<EOF
{
  "version": "1.0",
  "registry_url": "https://raw.githubusercontent.com/yoonhoGo/conveyor/main/registry.json",
  "plugins": {
    "http": {
      "name": "http",
      "version": "0.2.0",
      "description": "HTTP source and sink plugin for REST API integration",
      "author": "Conveyor Team",
      "repository": "https://github.com/yoonhoGo/conveyor",
      "downloads": {
        "darwin-aarch64": {
          "url": "https://github.com/yoonhoGo/conveyor/releases/download/$VERSION/libconveyor_plugin_http.dylib",
          "checksum": "sha256:$HTTP_CHECKSUM"
        }
      }
    },
    "mongodb": {
      "name": "mongodb",
      "version": "0.2.0",
      "description": "MongoDB source and sink plugin for database operations",
      "author": "Conveyor Team",
      "repository": "https://github.com/yoonhoGo/conveyor",
      "downloads": {
        "darwin-aarch64": {
          "url": "https://github.com/yoonhoGo/conveyor/releases/download/$VERSION/libconveyor_plugin_mongodb.dylib",
          "checksum": "sha256:$MONGO_CHECKSUM"
        }
      }
    }
  }
}
EOF

echo "✅ registry.json updated with checksums"

echo ""
echo "✅ Deployment complete!"
echo "========================================"
echo ""
echo "Next steps:"
echo "1. Review the draft release at: https://github.com/$REPO/releases/tag/$VERSION"
echo "2. Commit and push registry.json:"
echo "   git add registry.json"
echo "   git commit -m 'chore: update registry with checksums for $VERSION'"
echo "   git push"
echo "3. Publish the release on GitHub"
echo ""
echo "Users can then install plugins with:"
echo "   conveyor plugin install http"
echo "   conveyor plugin install mongodb"
