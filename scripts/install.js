#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Get platform-specific binary name
function getPlatformBinary() {
  const platform = process.platform;
  const arch = process.arch;

  const binaryMap = {
    'darwin-arm64': 'conveyor-darwin-arm64',
    'darwin-x64': 'conveyor-darwin-x64',
    'linux-x64': 'conveyor-linux-x64',
    'linux-arm64': 'conveyor-linux-arm64',
    'win32-x64': 'conveyor-win32-x64.exe'
  };

  const key = `${platform}-${arch}`;
  return binaryMap[key];
}

// Try to find binary from optional dependencies
function findBinaryFromOptionalDeps() {
  const binaryName = getPlatformBinary();
  if (!binaryName) {
    console.error(`Unsupported platform: ${process.platform}-${process.arch}`);
    process.exit(1);
  }

  const packageName = `@conveyor/cli-${process.platform}-${process.arch}`;

  try {
    const packagePath = require.resolve(`${packageName}/package.json`);
    const packageDir = path.dirname(packagePath);
    const binaryPath = path.join(packageDir, binaryName);

    if (fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  } catch (e) {
    // Optional dependency not found, try building from source
  }

  return null;
}

// Build from source as fallback
function buildFromSource() {
  console.log('Building conveyor from source...');

  try {
    execSync('cargo build --release', { stdio: 'inherit' });

    const binaryName = process.platform === 'win32' ? 'conveyor.exe' : 'conveyor';
    const sourcePath = path.join(__dirname, '..', 'target', 'release', binaryName);

    if (fs.existsSync(sourcePath)) {
      return sourcePath;
    }
  } catch (e) {
    console.error('Failed to build from source:', e.message);
    console.error('Please install Rust: https://rustup.rs/');
    process.exit(1);
  }

  return null;
}

// Main installation logic
function install() {
  const binDir = path.join(__dirname, '..', 'bin');
  const targetBinary = path.join(binDir, process.platform === 'win32' ? 'conveyor.exe' : 'conveyor');

  // Ensure bin directory exists
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  // Try to find binary from optional dependencies
  let sourceBinary = findBinaryFromOptionalDeps();

  // Fallback to building from source
  if (!sourceBinary) {
    console.log('Platform-specific binary not found, building from source...');
    sourceBinary = buildFromSource();
  }

  if (!sourceBinary) {
    console.error('Failed to install conveyor binary');
    process.exit(1);
  }

  // Copy binary to bin directory
  fs.copyFileSync(sourceBinary, targetBinary);

  // Make executable on Unix-like systems
  if (process.platform !== 'win32') {
    fs.chmodSync(targetBinary, 0o755);
  }

  console.log('✅ Conveyor installed successfully!');
  console.log(`Binary location: ${targetBinary}`);
}

install();
