#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

BIN_DIR="/usr/local/bin"
CONFIG_DIR="/etc/opensim"
DATA_DIR="/var/lib/opensim"
SERVICE="welkin-robust.service"
SERVICE_FILE="/etc/systemd/system/$SERVICE"

info()  { printf "${BLUE}[INFO]${NC}  %s\n" "$*"; }
ok()    { printf "${GREEN}[OK]${NC}    %s\n" "$*"; }
warn()  { printf "${YELLOW}[WARN]${NC}  %s\n" "$*"; }

check_root() {
    if [[ $EUID -ne 0 ]]; then
        printf "${RED}[ERROR]${NC} %s\n" "This script must be run as root" >&2
        exit 1
    fi
}

stop_service() {
    if systemctl is-active --quiet "$SERVICE" 2>/dev/null; then
        info "Stopping service..."
        systemctl stop "$SERVICE"
    fi
    if systemctl is-enabled --quiet "$SERVICE" 2>/dev/null; then
        systemctl disable "$SERVICE"
    fi
    ok "Service stopped and disabled"
}

remove_service() {
    if [[ -f "$SERVICE_FILE" ]]; then
        rm "$SERVICE_FILE"
        systemctl daemon-reload
        ok "Service file removed"
    fi
}

remove_binary() {
    if [[ -f "$BIN_DIR/welkin-robust" ]]; then
        rm "$BIN_DIR/welkin-robust"
        ok "Binary removed"
    fi
}

print_kept() {
    echo ""
    warn "The following directories were NOT removed (contain your world data):"
    echo ""
    if [[ -d "$DATA_DIR" ]]; then
        info "  Data:   $DATA_DIR/"
    fi
    if [[ -d "$CONFIG_DIR" ]]; then
        info "  Config: $CONFIG_DIR/"
    fi
    echo ""
    info "To remove all data:  rm -rf $DATA_DIR $CONFIG_DIR"
    info "To remove the user:  userdel opensim"
}

main() {
    echo ""
    info "OpenSimulator Uninstall"
    echo ""

    check_root
    stop_service
    remove_service
    remove_binary
    print_kept
}

main "$@"
