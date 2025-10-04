#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# GitHub repository information
REPO="yoonhoGo/conveyor"
BINARY_NAME="conveyor"

# Detect OS and architecture (macOS only)
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    # Check if running on macOS
    if [[ "$os" != "darwin"* ]]; then
        echo -e "${RED}This installer only supports macOS${NC}"
        echo -e "${YELLOW}For other platforms, please build from source${NC}"
        exit 1
    fi

    OS="darwin"

    case "$arch" in
        x86_64|amd64)
            ARCH="x64"
            ;;
        aarch64|arm64)
            ARCH="arm64"
            ;;
        *)
            echo -e "${RED}Unsupported architecture: $arch${NC}"
            exit 1
            ;;
    esac

    PLATFORM="${OS}-${ARCH}"
}

# Get latest release version from GitHub
get_latest_version() {
    echo -e "${YELLOW}Fetching latest release...${NC}" >&2
    VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        echo -e "${RED}Failed to fetch latest version${NC}" >&2
        exit 1
    fi

    echo -e "${GREEN}Latest version: ${VERSION}${NC}" >&2
}

# Download binary
download_binary() {
    local binary_name="${BINARY_NAME}-${PLATFORM}"
    local download_url="https://github.com/${REPO}/releases/download/${VERSION}/${binary_name}"
    local temp_file="/tmp/${binary_name}"

    echo -e "${YELLOW}Downloading from: ${download_url}${NC}" >&2

    if ! curl -L -o "$temp_file" "$download_url"; then
        echo -e "${RED}Failed to download binary${NC}" >&2
        exit 1
    fi

    echo "$temp_file"
}

# Install binary
install_binary() {
    local temp_file=$1
    local install_dir="${INSTALL_DIR:-/usr/local/bin}"

    # Try to create install directory if it doesn't exist
    if [ ! -d "$install_dir" ]; then
        echo -e "${YELLOW}Creating directory: ${install_dir}${NC}"
        mkdir -p "$install_dir" 2>/dev/null || {
            echo -e "${YELLOW}Cannot create ${install_dir}, falling back to ~/.local/bin${NC}"
            install_dir="$HOME/.local/bin"
            mkdir -p "$install_dir"
        }
    fi

    local install_path="${install_dir}/${BINARY_NAME}"

    # Make binary executable
    chmod +x "$temp_file"

    # Remove quarantine attribute and apply ad-hoc signature
    echo -e "${YELLOW}Removing quarantine and applying ad-hoc signature...${NC}"
    xattr -d com.apple.quarantine "$temp_file" 2>/dev/null || true
    codesign --force --sign - "$temp_file" 2>/dev/null || true

    # Try to move binary to install directory
    echo -e "${YELLOW}Installing to: ${install_path}${NC}"
    if mv "$temp_file" "$install_path" 2>/dev/null; then
        echo -e "${GREEN}Successfully installed ${BINARY_NAME} to ${install_path}${NC}"
    else
        echo -e "${YELLOW}Permission denied. Attempting with sudo...${NC}"
        if sudo mv "$temp_file" "$install_path" 2>/dev/null; then
            echo -e "${GREEN}Successfully installed ${BINARY_NAME} to ${install_path}${NC}"
        else
            echo -e "${YELLOW}sudo failed, falling back to ~/.local/bin${NC}"
            install_dir="$HOME/.local/bin"
            install_path="${install_dir}/${BINARY_NAME}"
            mkdir -p "$install_dir"
            mv "$temp_file" "$install_path"
            echo -e "${GREEN}Successfully installed ${BINARY_NAME} to ${install_path}${NC}"
        fi
    fi

    # Check if install directory is in PATH
    if [[ ":$PATH:" != *":$install_dir:"* ]]; then
        echo -e "${YELLOW}Warning: ${install_dir} is not in your PATH${NC}"
        echo -e "${YELLOW}Add this to your shell profile (~/.bashrc or ~/.zshrc):${NC}"
        echo -e "${GREEN}export PATH=\"${install_dir}:\$PATH\"${NC}"
    fi
}

# Verify installation
verify_installation() {
    echo -e "${YELLOW}Verifying installation...${NC}"
    if command -v "$BINARY_NAME" &> /dev/null; then
        local installed_version=$("$BINARY_NAME" --version 2>&1 | head -n1 || echo "unknown")
        echo -e "${GREEN}Installation successful!${NC}"
        echo -e "${GREEN}Version: ${installed_version}${NC}"
        echo -e "${GREEN}Run '${BINARY_NAME} --help' to get started${NC}"
    else
        echo -e "${YELLOW}Binary installed but not found in PATH${NC}"
        echo -e "${YELLOW}You may need to restart your shell or add the installation directory to PATH${NC}"
    fi
}

# Main installation flow
main() {
    echo -e "${GREEN}Conveyor Installation Script${NC}"
    echo ""

    detect_platform
    echo -e "${GREEN}Detected platform: ${PLATFORM}${NC}"

    get_latest_version

    temp_file=$(download_binary)

    install_binary "$temp_file"

    verify_installation

    echo ""
    echo -e "${GREEN}Thank you for installing Conveyor!${NC}"
}

# Run main function
main
