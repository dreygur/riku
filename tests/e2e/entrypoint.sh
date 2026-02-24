#!/bin/bash
export PATH="/root:/root/.local/bin:$PATH"
exec /usr/sbin/sshd -D
