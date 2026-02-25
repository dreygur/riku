#!/bin/bash
export PATH="/root:/root/.local/bin:$PATH"

# Start nginx
/usr/sbin/nginx

# Start SSH daemon
exec /usr/sbin/sshd -D
