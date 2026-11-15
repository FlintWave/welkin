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

info()  { printf "${BLUE}[INFO]${NC}  %s\n" "$*"; }
ok()    { printf "${GREEN}[OK]${NC}    %s\n" "$*"; }
warn()  { printf "${YELLOW}[WARN]${NC}  %s\n" "$*"; }
err()   { printf "${RED}[ERROR]${NC} %s\n" "$*" >&2; }

check_root() {
    if [[ $EUID -ne 0 ]]; then
        err "This script must be run as root"
        exit 1
    fi
}

check_existing() {
    if [[ ! -f "$BIN_DIR/welkin-robust" ]]; then
        err "welkin-robust not found at $BIN_DIR/welkin-robust"
        err "Run install.sh first"
        exit 1
    fi
}

backup_config() {
    local backup_dir
    backup_dir="$DATA_DIR/backups/upgrade-$(date +%Y%m%d-%H%M%S)"
    mkdir -p "$backup_dir"
    cp -a "$CONFIG_DIR/" "$backup_dir/config"
    ok "Config backed up to: $backup_dir/config"
}

stop_service() {
    if systemctl is-active --quiet "$SERVICE"; then
        info "Stopping service..."
        systemctl stop "$SERVICE"
        ok "Service stopped"
    else
        info "Service was not running"
    fi
}

replace_binary() {
    local new_binary="${1:-}"
    if [[ -z "$new_binary" ]]; then
        err "Usage: upgrade.sh <path-to-new-binary>"
        exit 1
    fi
    if [[ ! -f "$new_binary" ]]; then
        err "Binary not found: $new_binary"
        exit 1
    fi

    cp "$new_binary" "$BIN_DIR/welkin-robust"
    chmod 755 "$BIN_DIR/welkin-robust"
    ok "Binary replaced"
}

run_migrations() {
    info "Running database migrations..."
    sudo -u opensim "$BIN_DIR/welkin-robust" --config "$CONFIG_DIR/robust.ini" --migrate || {
        warn "Migration returned non-zero; check logs"
    }
    ok "Migrations complete"
}

start_service() {
    systemctl daemon-reload
    systemctl start "$SERVICE"

    sleep 2
    if systemctl is-active --quiet "$SERVICE"; then
        ok "Service restarted"
    else
        warn "Service may not have started; check: journalctl -u $SERVICE"
    fi
}

main() {
    echo ""
    info "OpenSimulator Upgrade"
    echo ""

    check_root
    check_existing
    backup_config
    stop_service
    replace_binary "${1:-}"
    run_migrations
    start_service

    echo ""
    ok "Upgrade complete. Data preserved at $DATA_DIR/"
}

main "$@"
