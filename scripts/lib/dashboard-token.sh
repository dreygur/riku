# Sourced by the dashboard scripts. The tunnel script starts the backend
# with this token and the web script sends it, so both read one file
# instead of the token being copied between terminals.

DASHBOARD_TOKEN_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/riku/dashboard-token"

# Prints the token, creating it on first use. The file is mode 600.
dashboard_token() {
    if [ ! -s "$DASHBOARD_TOKEN_FILE" ]; then
        mkdir -p "$(dirname "$DASHBOARD_TOKEN_FILE")"
        (umask 077 && openssl rand -hex 32 > "$DASHBOARD_TOKEN_FILE")
    fi
    cat "$DASHBOARD_TOKEN_FILE"
}
