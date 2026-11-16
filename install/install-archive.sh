#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-3.0-or-later
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

INSTALL_DIR="${1:-/opt/opensim}"
DATA_DIR="/var/lib/opensim"
CONFIG_DIR="/etc/opensim"
SERVICE="welkin-robust.service"
SERVICE_FILE="/etc/systemd/system/$SERVICE"

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

find_archive() {
    ARCHIVE="${2:-}"
    if [[ -z "$ARCHIVE" ]]; then
        ARCHIVE="$(ls -1t opensim-*.tar.gz 2>/dev/null | head -1)" || true
    fi
    if [[ -z "$ARCHIVE" || ! -f "$ARCHIVE" ]]; then
        err "Usage: install-archive.sh [install-dir] <archive.tar.gz>"
        err "Or place opensim-*.tar.gz in the current directory"
        exit 1
    fi
    info "Using archive: $ARCHIVE"
}

create_user() {
    if ! id opensim &>/dev/null; then
        useradd --system --home-dir "$DATA_DIR" --shell /usr/sbin/nologin opensim
        ok "Created system user: opensim"
    else
        ok "User opensim already exists"
    fi
}

extract_archive() {
    mkdir -p "$INSTALL_DIR"
    tar -xzf "$ARCHIVE" -C "$INSTALL_DIR" --strip-components=1
    chmod 755 "$INSTALL_DIR/bin/welkin-robust"
    ok "Extracted to: $INSTALL_DIR"
}

create_directories() {
    mkdir -p "$DATA_DIR"/{data,logs,backups}
    mkdir -p "$CONFIG_DIR"
    chown -R opensim:opensim "$DATA_DIR"
    ok "Data directories ready"
}

generate_config() {
    local config_file="$CONFIG_DIR/robust.ini"
    if [[ -f "$config_file" ]]; then
        ok "Config already exists: $config_file"
        return
    fi

    cat > "$config_file" << CONF
[Startup]
ConfigDirectory = $CONFIG_DIR
DataDirectory = $DATA_DIR/data
LogDirectory = $DATA_DIR/logs

[Network]
HttpListenerPort = 8002
GridServicePort = 8003

[DatabaseService]
StorageProvider = sqlite
ConnectionString = Data Source=$DATA_DIR/data/opensim.db

[GridService]
Region_Welcome = DefaultRegion

[AuthenticationService]
AllowGetAuthInfo = false
AllowSetAuthInfo = false
AllowSetPassword = false
CONF

    chown opensim:opensim "$config_file"
    chmod 640 "$config_file"
    ok "Generated config: $config_file"
}

generate_admin_token() {
    local token_file="$CONFIG_DIR/admin-token"
    if [[ -f "$token_file" ]]; then
        ok "Admin token already exists"
        return
    fi

    local token
    if command -v uuidgen &>/dev/null; then
        token="$(uuidgen)"
    else
        token="$(cat /proc/sys/kernel/random/uuid)"
    fi
    printf '%s' "$token" > "$token_file"
    chown opensim:opensim "$token_file"
    chmod 600 "$token_file"
    ok "Generated admin token"
}

install_service() {
    cat > "$SERVICE_FILE" << UNIT
[Unit]
Description=OpenSimulator Grid Services
After=network.target

[Service]
Type=simple
User=opensim
Group=opensim
ExecStart=$INSTALL_DIR/bin/welkin-robust --config $CONFIG_DIR/robust.ini
WorkingDirectory=$DATA_DIR
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
UNIT

    systemctl daemon-reload
    systemctl enable "$SERVICE"
    ok "Systemd service installed"
}

start_service() {
    systemctl start "$SERVICE"

    sleep 2
    if systemctl is-active --quiet "$SERVICE"; then
        ok "Service started"
    else
        warn "Service may not have started; check: journalctl -u $SERVICE"
    fi
}

print_summary() {
    local ip
    ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
    ip="${ip:-localhost}"

    echo ""
    printf "${GREEN}════════════════════════════════════════════════════${NC}\n"
    printf "${GREEN}  OpenSimulator installed from archive${NC}\n"
    printf "${GREEN}════════════════════════════════════════════════════${NC}\n"
    echo ""
    info "Install:      $INSTALL_DIR/"
    info "Config:       $CONFIG_DIR/robust.ini"
    info "Data:         $DATA_DIR/"
    info "Admin token:  $CONFIG_DIR/admin-token"
    echo ""
}

main() {
    echo ""
    info "OpenSimulator Archive Installer"
    echo ""

    check_root
    find_archive "$@"
    create_user
    extract_archive
    create_directories
    generate_config
    generate_admin_token
    install_service
    start_service
    print_summary
}

main "$@"
